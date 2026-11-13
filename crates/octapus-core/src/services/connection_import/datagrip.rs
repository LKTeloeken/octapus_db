//! DataGrip: `dataSources.xml` (compartilhado) + `dataSources.local.xml`
//! (usuário, SSH, SSL) de cada lugar onde a JetBrains guarda data sources — o
//! global (`<config>/options`) e o `.idea` de cada projeto. Host, porta e
//! database saem da URL JDBC. Sem senhas: ficam no cofre da JetBrains.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use quick_xml::events::{BytesStart, Event};
use quick_xml::{Reader, XmlVersion};

use super::mapping::RawConnection;
use super::{paths, url};
use crate::error::{Error, Result};

/// Os `dataSources.xml` desta máquina (versão mais nova do DataGrip primeiro).
pub fn find_files() -> Vec<PathBuf> {
    let Some(root) = paths::jetbrains_root() else {
        return Vec::new();
    };
    let mut configs: Vec<PathBuf> = std::fs::read_dir(&root)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_dir()
                && path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with("DataGrip"))
        })
        .collect();
    configs.sort();
    configs.reverse();

    let mut dirs: Vec<PathBuf> = Vec::new();
    for config in &configs {
        dirs.push(config.join("options"));
        dirs.extend(recent_projects(config).into_iter().map(|p| p.join(".idea")));
    }
    // Pasta padrão dos projetos do DataGrip
    if let Some(home) = dirs::home_dir() {
        let projects = std::fs::read_dir(home.join("DataGripProjects"))
            .into_iter()
            .flatten()
            .flatten();
        dirs.extend(projects.map(|entry| entry.path().join(".idea")));
    }

    let mut files = Vec::new();
    for dir in dirs {
        let file = dir.join("dataSources.xml");
        if file.is_file() && !files.contains(&file) {
            files.push(file);
        }
    }
    files
}

/// Caminhos de `options/recentProjects.xml` (`$USER_HOME$` expandido).
fn recent_projects(config: &Path) -> Vec<PathBuf> {
    let Ok(text) = std::fs::read_to_string(config.join("options/recentProjects.xml")) else {
        return Vec::new();
    };
    let home = dirs::home_dir().unwrap_or_default();
    let mut reader = Reader::from_str(&text);
    let mut out = Vec::new();
    while let Ok(event) = reader.read_event() {
        match event {
            Event::Start(e) | Event::Empty(e) if e.name().as_ref() == b"entry" => {
                if let Some(key) = attribute(&e, "key") {
                    out.push(PathBuf::from(
                        key.replace("$USER_HOME$", &home.to_string_lossy()),
                    ));
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    out
}

/// Lê cada `dataSources.xml` junto do `.local.xml` da mesma pasta, e resolve o
/// host do túnel SSH pelo `sshConfigs.xml` do DataGrip.
pub fn read(files: &[PathBuf]) -> Result<Vec<RawConnection>> {
    let ssh_hosts = ssh_config_hosts();
    let mut out = Vec::new();
    for file in files {
        let shared = std::fs::read_to_string(file).map_err(|e| {
            Error::InvalidState(format!("Não deu para ler {}: {e}", file.display()))
        })?;
        let local = file
            .parent()
            .and_then(|dir| std::fs::read_to_string(dir.join("dataSources.local.xml")).ok());
        out.extend(parse(&shared, local.as_deref(), &ssh_hosts));
    }
    Ok(out)
}

/// `id → host` de `options/sshConfigs.xml` de todas as versões.
fn ssh_config_hosts() -> HashMap<String, String> {
    let mut hosts = HashMap::new();
    let Some(root) = paths::jetbrains_root() else {
        return hosts;
    };
    for entry in std::fs::read_dir(root).into_iter().flatten().flatten() {
        let Ok(text) = std::fs::read_to_string(entry.path().join("options/sshConfigs.xml")) else {
            continue;
        };
        let mut reader = Reader::from_str(&text);
        while let Ok(event) = reader.read_event() {
            match event {
                Event::Start(e) | Event::Empty(e) if e.name().as_ref() == b"sshConfig" => {
                    if let (Some(id), Some(host)) = (attribute(&e, "id"), attribute(&e, "host")) {
                        hosts.insert(id, host);
                    }
                }
                Event::Eof => break,
                _ => {}
            }
        }
    }
    hosts
}

/// O que um arquivo diz de um data source; o compartilhado e o local se somam.
#[derive(Debug, Default, Clone)]
struct Fragment {
    name: Option<String>,
    group: Option<String>,
    jdbc_url: Option<String>,
    user: Option<String>,
    ssh_enabled: bool,
    ssh_config: Option<String>,
    ssh_host: Option<String>,
    ssl_enabled: bool,
    ssl_mode: Option<String>,
    client_cert: bool,
}

impl Fragment {
    fn merge(&mut self, other: Fragment) {
        self.name = other.name.or(self.name.take());
        self.group = other.group.or(self.group.take());
        self.jdbc_url = other.jdbc_url.or(self.jdbc_url.take());
        self.user = other.user.or(self.user.take());
        self.ssh_enabled |= other.ssh_enabled;
        self.ssh_config = other.ssh_config.or(self.ssh_config.take());
        self.ssh_host = other.ssh_host.or(self.ssh_host.take());
        self.ssl_enabled |= other.ssl_enabled;
        self.ssl_mode = other.ssl_mode.or(self.ssl_mode.take());
        self.client_cert |= other.client_cert;
    }
}

pub fn parse(
    shared: &str,
    local: Option<&str>,
    ssh_hosts: &HashMap<String, String>,
) -> Vec<RawConnection> {
    let mut order: Vec<String> = Vec::new();
    let mut fragments: HashMap<String, Fragment> = HashMap::new();
    for xml in std::iter::once(shared).chain(local) {
        for (uuid, fragment) in parse_fragments(xml) {
            match fragments.get_mut(&uuid) {
                Some(existing) => existing.merge(fragment),
                None => {
                    order.push(uuid.clone());
                    fragments.insert(uuid, fragment);
                }
            }
        }
    }

    order
        .iter()
        .filter_map(|uuid| {
            let fragment = fragments.remove(uuid)?;
            let (driver, parsed) = url::parse_jdbc(fragment.jdbc_url.as_deref()?)?;
            let mut raw = RawConnection {
                username: fragment.user.clone().unwrap_or(parsed.username.clone()),
                group: fragment.group.clone(),
                ..RawConnection::new(
                    fragment.name.clone().unwrap_or_else(|| uuid.clone()),
                    driver,
                )
            };
            raw.host = parsed.host;
            raw.port = parsed.port;
            raw.database = parsed.database;
            raw.uri = parsed.uri;
            raw.ssl_mode = if fragment.ssl_enabled {
                Some(
                    fragment
                        .ssl_mode
                        .clone()
                        .unwrap_or_else(|| "require".into()),
                )
            } else {
                parsed.ssl_mode
            };
            raw.client_certificate = fragment.client_cert;
            if fragment.ssh_enabled {
                raw.ssh_host = Some(
                    fragment
                        .ssh_host
                        .clone()
                        .or_else(|| {
                            fragment
                                .ssh_config
                                .as_ref()
                                .and_then(|id| ssh_hosts.get(id).cloned())
                        })
                        .unwrap_or_default(),
                );
            }
            Some(raw)
        })
        .collect()
}

/// `(uuid, fragmento)` de cada `<data-source>` do arquivo.
fn parse_fragments(xml: &str) -> Vec<(String, Fragment)> {
    let mut reader = Reader::from_str(xml);
    let mut out = Vec::new();
    // Pilha dos elementos abertos dentro do data source atual
    let mut stack: Vec<String> = Vec::new();
    let mut current: Option<(String, Fragment)> = None;
    let mut text = String::new();

    loop {
        let event = match reader.read_event() {
            Ok(Event::Eof) | Err(_) => break,
            Ok(event) => event,
        };
        match &event {
            Event::Start(e) | Event::Empty(e) if e.name().as_ref() == b"data-source" => {
                let fragment = Fragment {
                    name: attribute(e, "name"),
                    group: attribute(e, "group-name"),
                    ..Fragment::default()
                };
                let Some(uuid) = attribute(e, "uuid") else {
                    continue;
                };
                if matches!(event, Event::Empty(_)) {
                    out.push((uuid, fragment));
                } else {
                    current = Some((uuid, fragment));
                    stack.clear();
                }
            }
            Event::End(e) if e.name().as_ref() == b"data-source" => {
                out.extend(current.take());
            }
            Event::Start(e) if current.is_some() => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                if let Some((_, fragment)) = current.as_mut() {
                    on_attributes(fragment, &name, e);
                }
                stack.push(name);
                text.clear();
            }
            Event::Empty(e) if current.is_some() => {
                let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                if let Some((_, fragment)) = current.as_mut() {
                    on_attributes(fragment, &name, e);
                }
            }
            Event::Text(t) if current.is_some() => {
                text.push_str(&t.decode().unwrap_or_default());
            }
            Event::GeneralRef(r) if current.is_some() => {
                if let Ok(Some(c)) = r.resolve_char_ref() {
                    text.push(c);
                } else {
                    text.push_str(match r.decode().unwrap_or_default().as_ref() {
                        "amp" => "&",
                        "lt" => "<",
                        "gt" => ">",
                        "quot" => "\"",
                        "apos" => "'",
                        _ => "",
                    });
                }
            }
            Event::End(_) if current.is_some() => {
                let element = stack.pop().unwrap_or_default();
                let parent = stack.last().map(String::as_str).unwrap_or_default();
                let value = text.trim().to_string();
                text.clear();
                if let Some((_, fragment)) = current.as_mut() {
                    on_text(fragment, parent, &element, value);
                }
            }
            _ => {}
        }
    }
    out
}

/// Campos que o DataGrip grava como texto de elemento.
fn on_text(fragment: &mut Fragment, parent: &str, element: &str, value: String) {
    if value.is_empty() {
        return;
    }
    match (parent, element) {
        (_, "jdbc-url") => fragment.jdbc_url = Some(value),
        (_, "user-name") => fragment.user = Some(value),
        ("ssh-properties", "enabled") => fragment.ssh_enabled = value == "true",
        ("ssh-properties", "ssh-config-id") => fragment.ssh_config = Some(value),
        ("ssl-config", "enabled") => fragment.ssl_enabled = value == "true",
        ("ssl-config", "mode") => {
            fragment.ssl_mode = Some(value.to_ascii_lowercase().replace('_', "-"))
        }
        ("ssl-config", "client-cert") => fragment.client_cert = true,
        _ => {}
    }
}

/// Versões antigas gravam SSH como atributos de `<ssh-properties>`.
fn on_attributes(fragment: &mut Fragment, element: &str, e: &BytesStart) {
    if element == "ssh-properties" {
        if attribute(e, "enabled").as_deref() == Some("true") {
            fragment.ssh_enabled = true;
        }
        fragment.ssh_host = attribute(e, "host").or(fragment.ssh_host.take());
        fragment.ssh_config = attribute(e, "ssh-config-id").or(fragment.ssh_config.take());
    }
}

fn attribute(e: &BytesStart, name: &str) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|attr| attr.key.as_ref() == name.as_bytes())
        .and_then(|attr| attr.normalized_value(XmlVersion::Implicit1_0).ok())
        .map(|value| value.into_owned())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHARED: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<project version="4">
  <component name="DataSourceManagerImpl" format="xml" multifile-model="true">
    <data-source source="LOCAL" name="ERP" group-name="Clientes" uuid="u-1">
      <driver-ref>postgresql</driver-ref>
      <jdbc-url>jdbc:postgresql://db.erp:5433/erp?ApplicationName=x&amp;sslmode=prefer</jdbc-url>
    </data-source>
    <data-source source="LOCAL" name="Notas" uuid="u-2">
      <driver-ref>sqlite.xerial</driver-ref>
      <jdbc-url>jdbc:sqlite:/Users/voce/notas.db</jdbc-url>
    </data-source>
    <data-source source="LOCAL" name="Sem URL" uuid="u-3" />
  </component>
</project>"#;

    const LOCAL: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<project version="4">
  <component name="dataSourceStorageLocal">
    <data-source name="ERP" uuid="u-1">
      <user-name>erp</user-name>
      <ssh-properties>
        <enabled>true</enabled>
        <ssh-config-id>cfg-1</ssh-config-id>
      </ssh-properties>
      <ssl-config>
        <enabled>true</enabled>
        <mode>VERIFY_FULL</mode>
      </ssl-config>
    </data-source>
  </component>
</project>"#;

    #[test]
    fn merges_shared_and_local_fragments() {
        let ssh = HashMap::from([("cfg-1".to_string(), "bastion.erp".to_string())]);
        let raws = parse(SHARED, Some(LOCAL), &ssh);
        assert_eq!(raws.len(), 2, "data source sem URL fica de fora");

        let erp = &raws[0];
        assert_eq!(
            (erp.name.as_str(), erp.driver.as_str()),
            ("ERP", "PostgreSQL")
        );
        assert_eq!((erp.host.as_str(), erp.port), ("db.erp", Some(5433)));
        assert_eq!(erp.database.as_deref(), Some("erp"));
        assert_eq!(erp.username, "erp");
        assert_eq!(erp.group.as_deref(), Some("Clientes"));
        assert_eq!(erp.ssh_host.as_deref(), Some("bastion.erp"));
        assert_eq!(erp.ssl_mode.as_deref(), Some("verify-full"));

        let notas = &raws[1];
        assert_eq!(notas.driver, "SQLite");
        assert_eq!(notas.database.as_deref(), Some("/Users/voce/notas.db"));
    }

    #[test]
    fn entity_in_url_does_not_break_parsing() {
        let raws = parse(SHARED, None, &HashMap::new());
        // `&amp;` no meio da URL: o sslmode depois dele ainda é lido
        assert_eq!(raws[0].ssl_mode.as_deref(), Some("prefer"));
        assert_eq!(raws[0].ssh_host, None);
    }
}
