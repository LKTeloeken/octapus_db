//! DBeaver: `data-sources*.json` de cada projeto do workspace e as senhas do
//! `credentials-config.json` ao lado (cifrado com chave fixa).
//!
//! Layout: `<raiz>/workspace6/<projeto>/.dbeaver/data-sources.json`.

use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use super::mapping::{canonical_driver, RawConnection, Secret};
use super::{crypto, paths};
use crate::error::{Error, Result};

/// Os `data-sources*.json` de todos os projetos do workspace mais novo.
pub fn find_files() -> Vec<PathBuf> {
    let Some(root) = paths::dbeaver_root() else {
        return Vec::new();
    };
    let mut workspaces: Vec<PathBuf> = read_dir(&root)
        .filter(|path| {
            path.is_dir()
                && path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with("workspace"))
        })
        .collect();
    // `workspace6` antes de `workspace5`: o DBeaver migra para o mais novo
    workspaces.sort();
    let Some(workspace) = workspaces.pop() else {
        return Vec::new();
    };

    let mut files: Vec<PathBuf> = read_dir(&workspace)
        .flat_map(|project| read_dir(&project.join(".dbeaver")))
        .filter(|path| {
            path.file_name().is_some_and(|name| {
                let name = name.to_string_lossy();
                name.starts_with("data-sources") && name.ends_with(".json")
            })
        })
        .collect();
    files.sort();
    files
}

fn read_dir(dir: &Path) -> impl Iterator<Item = PathBuf> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
}

/// Conexões de um ou mais `data-sources*.json` (cada um com o
/// `credentials-config.json` da mesma pasta).
pub fn read(files: &[PathBuf]) -> Result<Vec<RawConnection>> {
    let mut connections = Vec::new();
    for file in files {
        let text = std::fs::read_to_string(file).map_err(|e| {
            Error::InvalidState(format!("Não deu para ler {}: {e}", file.display()))
        })?;
        let credentials = file
            .parent()
            .map(|dir| dir.join("credentials-config.json"))
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|data| crypto::decrypt_dbeaver(&data))
            .and_then(|plain| serde_json::from_slice::<Value>(&plain).ok());
        connections.extend(parse(&text, credentials.as_ref())?);
    }
    Ok(connections)
}

pub fn parse(data_sources: &str, credentials: Option<&Value>) -> Result<Vec<RawConnection>> {
    let json: Value = serde_json::from_str(data_sources)
        .map_err(|e| Error::InvalidState(format!("data-sources.json inválido: {e}")))?;
    let Some(connections) = json.get("connections").and_then(Value::as_object) else {
        return Ok(Vec::new());
    };

    Ok(connections
        .iter()
        .filter_map(|(id, entry)| {
            let entry = entry.as_object()?;
            let config = entry.get("configuration").and_then(Value::as_object);
            let creds = credentials
                .and_then(|c| c.get(id))
                .and_then(|c| c.get("#connection"));
            Some(connection(id, entry, config, creds))
        })
        .collect())
}

fn connection(
    id: &str,
    entry: &Map<String, Value>,
    config: Option<&Map<String, Value>>,
    creds: Option<&Value>,
) -> RawConnection {
    let text = |key: &str| {
        config
            .and_then(|c| c.get(key))
            .and_then(Value::as_str)
            .map(str::to_string)
            .filter(|s| !s.is_empty())
    };
    let name = entry.get("name").and_then(Value::as_str).unwrap_or(id);

    let mut raw = RawConnection {
        host: text("host").unwrap_or_default(),
        port: config.and_then(|c| c.get("port")).and_then(port),
        username: creds
            .and_then(|c| c.get("user"))
            .and_then(Value::as_str)
            .map(str::to_string)
            .filter(|s| !s.is_empty())
            .or_else(|| text("user"))
            .unwrap_or_default(),
        database: text("database"),
        // Pasta aninhada: "Clientes/ACME" vira o grupo "Clientes / ACME"
        group: entry
            .get("folder")
            .and_then(Value::as_str)
            .filter(|f| !f.is_empty())
            .map(|f| f.split('/').collect::<Vec<_>>().join(" / ")),
        ..RawConnection::new(name, driver(entry))
    };

    // SQLite (driver "generic"): o arquivo vem em `database` ou na URL JDBC
    if raw.driver == "SQLite" && raw.database.is_none() {
        raw.database = text("url").map(|url| url.trim_start_matches("jdbc:sqlite:").to_string());
    }

    if let Some(password) = creds
        .and_then(|c| c.get("password"))
        .and_then(Value::as_str)
    {
        if !password.is_empty() {
            raw.secret = Secret::Plain(password.to_string());
        }
    }

    let handlers = config
        .and_then(|c| c.get("handlers"))
        .and_then(Value::as_object);
    let handler = |name: &str| {
        handlers
            .and_then(|h| h.get(name))
            .and_then(Value::as_object)
            .filter(|h| h.get("enabled").and_then(Value::as_bool).unwrap_or(false))
    };
    if let Some(ssh) = handler("ssh_tunnel") {
        let props = ssh.get("properties").and_then(Value::as_object);
        raw.ssh_host = Some(
            props
                .and_then(|p| p.get("host"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        );
    }
    // SSL do Postgres: `handlers.postgre_ssl` (o DBeaver também aceita `ssl`)
    if let Some(ssl) = handler("postgre_ssl").or_else(|| handler("ssl")) {
        let props = ssl.get("properties").and_then(Value::as_object);
        let prop = |key: &str| props.and_then(|p| p.get(key)).and_then(Value::as_str);
        raw.ssl_mode = Some(prop("sslMode").unwrap_or("require").to_string());
        raw.client_certificate = prop("clientCertPath").or(prop("ssl.client.cert")).is_some();
    }
    raw
}

/// `provider` + `driver` → banco. O SQLite vem como provider `generic`.
fn driver(entry: &Map<String, Value>) -> String {
    let provider = entry
        .get("provider")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let driver = entry
        .get("driver")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if provider.eq_ignore_ascii_case("generic") && driver.to_ascii_lowercase().contains("sqlite") {
        return "SQLite".into();
    }
    canonical_driver(provider)
}

/// Porta como número ou texto.
fn port(value: &Value) -> Option<u16> {
    value
        .as_u64()
        .and_then(|n| u16::try_from(n).ok())
        .or_else(|| value.as_str()?.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::connection_import::crypto::test_support::encrypt_dbeaver;

    const DATA_SOURCES: &str = r#"{
        "folders": { "Clientes": {}, "Clientes/ACME": { "parent": "Clientes" } },
        "connections": {
            "postgres-jdbc-1": {
                "provider": "postgresql", "driver": "postgres-jdbc", "name": "ACME prod",
                "folder": "Clientes/ACME",
                "configuration": {
                    "host": "db.acme.com", "port": "5433", "database": "erp",
                    "handlers": {
                        "ssh_tunnel": { "enabled": true, "properties": { "host": "bastion.acme.com" } },
                        "postgre_ssl": { "enabled": true, "properties": { "sslMode": "verify-full" } }
                    }
                }
            },
            "sqlite-1": {
                "provider": "generic", "driver": "sqlite_jdbc", "name": "Notas",
                "configuration": { "url": "jdbc:sqlite:/Users/voce/notas.db" }
            },
            "mysql-1": {
                "provider": "mysql", "driver": "mysql8", "name": "Legado",
                "configuration": { "host": "localhost", "port": 3306, "user": "root" }
            }
        }
    }"#;

    #[test]
    fn reads_connections_folders_and_handlers() {
        let creds: Value = serde_json::from_str(
            r##"{"postgres-jdbc-1":{"#connection":{"user":"erp","password":"s3cret"}}}"##,
        )
        .unwrap();
        let raws = parse(DATA_SOURCES, Some(&creds)).unwrap();
        assert_eq!(raws.len(), 3);

        let pg = raws.iter().find(|r| r.name == "ACME prod").unwrap();
        assert_eq!(pg.driver, "PostgreSQL");
        assert_eq!((pg.host.as_str(), pg.port), ("db.acme.com", Some(5433)));
        assert_eq!(pg.username, "erp");
        assert_eq!(pg.database.as_deref(), Some("erp"));
        assert_eq!(pg.group.as_deref(), Some("Clientes / ACME"));
        assert_eq!(pg.ssh_host.as_deref(), Some("bastion.acme.com"));
        assert_eq!(pg.ssl_mode.as_deref(), Some("verify-full"));
        assert_eq!(pg.secret, Secret::Plain("s3cret".into()));

        let sqlite = raws.iter().find(|r| r.name == "Notas").unwrap();
        assert_eq!(sqlite.driver, "SQLite");
        assert_eq!(sqlite.database.as_deref(), Some("/Users/voce/notas.db"));

        let mysql = raws.iter().find(|r| r.name == "Legado").unwrap();
        assert_eq!(
            (mysql.driver.as_str(), mysql.username.as_str()),
            ("MySQL", "root")
        );
        assert_eq!(mysql.secret, Secret::None);
    }

    #[test]
    fn reads_files_with_encrypted_credentials() {
        let dir = std::env::temp_dir().join(format!("octapus-dbeaver-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("data-sources.json");
        std::fs::write(&file, DATA_SOURCES).unwrap();
        std::fs::write(
            dir.join("credentials-config.json"),
            encrypt_dbeaver(br##"{"mysql-1":{"#connection":{"password":"root!"}}}"##),
        )
        .unwrap();

        let raws = read(&[file]).unwrap();
        let mysql = raws.iter().find(|r| r.name == "Legado").unwrap();
        assert_eq!(mysql.secret, Secret::Plain("root!".into()));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn invalid_json_is_an_error_and_no_connections_is_empty() {
        assert!(parse("{", None).is_err());
        assert!(parse("{}", None).unwrap().is_empty());
    }
}
