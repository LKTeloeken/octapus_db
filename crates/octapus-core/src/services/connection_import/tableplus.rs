//! TablePlus. Cada sistema guarda as conexões de um jeito:
//!
//! - macOS (e Windows): `Connections.plist` + `ConnectionGroups.plist` na pasta
//!   `Data`, com as senhas no Keychain (serviço `com.tableplus.TablePlus`,
//!   conta `<ID>_database`) — lidas só no commit, ver `keychain.rs`;
//! - Linux: `~/.tableplus/settings/connections.json`, no formato
//!   `{"objects": [{"name", "db-driver", "db-host", "db-port", "db-user",
//!   "db-name", "db-path", "db-url", "ssh-host", "ssl-mode", "group-id"…}]}`,
//!   com tudo em texto (porta vazia = padrão). As senhas não ficam no arquivo
//!   (`db-command-line-password` é um comando, não a senha).
//!
//! Os dois viram o mesmo `serde_json::Value` e cada campo é procurado pelo nome
//! normalizado (sem caixa, `-` e `_`): `ConnectionName`, `connection-name` e
//! `connection_name` são a mesma chave; os nomes de cada formato são sinônimos
//! na mesma lista (`DatabaseHost` do plist, `db-host` do Linux).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use super::mapping::{canonical_driver, RawConnection, Secret};
use super::{paths, url};
use crate::error::{Error, Result};

pub const KEYCHAIN_SERVICE: &str = "com.tableplus.TablePlus";

/// O arquivo de conexões encontrado nesta máquina.
pub fn find_file() -> Option<PathBuf> {
    paths::tableplus_files()
        .into_iter()
        .find(|path| path.is_file())
}

/// Lê um `Connections.plist` ou um `connections.json` e, da mesma pasta, os
/// nomes dos grupos.
pub fn read(file: &Path) -> Result<Vec<RawConnection>> {
    let invalid = || {
        Error::InvalidState(format!(
            "{} não é um arquivo de conexões do TablePlus",
            file.display()
        ))
    };
    let (document, format) = load(file).ok_or_else(invalid)?;
    let connections = connection_list(&document).ok_or_else(invalid)?;

    let dir = file.parent();
    let groups = group_list(&document)
        .or_else(|| {
            [
                "ConnectionGroups.plist",
                "groups.json",
                "connection-groups.json",
            ]
            .iter()
            .find_map(|name| load(&dir?.join(name)))
            .and_then(|(doc, _)| doc.as_array().cloned())
        })
        .unwrap_or_default();

    // Só o plist do app de macOS guarda a senha no Keychain; o JSON do Linux,
    // mesmo lido num Mac, não tem senha nenhuma para buscar lá
    let keychain = format == Format::Plist && cfg!(target_os = "macos");
    Ok(parse(&connections, &groups, keychain))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    Json,
    Plist,
}

/// plist (XML ou binário) ou JSON, pelo conteúdo — não pela extensão.
fn load(path: &Path) -> Option<(Value, Format)> {
    let bytes = std::fs::read(path).ok()?;
    if let Ok(json) = serde_json::from_slice(&bytes) {
        return Some((json, Format::Json));
    }
    let plist = plist::Value::from_reader(std::io::Cursor::new(&bytes)).ok()?;
    Some((serde_json::to_value(plist).ok()?, Format::Plist))
}

/// A lista de conexões: o próprio documento (plist) ou uma lista dentro dele
/// (`{"objects": [...]}` no Linux).
fn connection_list(document: &Value) -> Option<Vec<Value>> {
    match document {
        Value::Array(items) => Some(items.clone()),
        Value::Object(map) => field(map, &["objects", "connections", "items"])?
            .as_array()
            .cloned(),
        _ => None,
    }
}

fn group_list(document: &Value) -> Option<Vec<Value>> {
    let map = document.as_object()?;
    field(map, &["groups", "connectiongroups"])?
        .as_array()
        .cloned()
}

/// `keychain`: no macOS a senha mora no Keychain; nos outros sistemas só vem se
/// estiver no próprio arquivo.
pub fn parse(connections: &[Value], groups: &[Value], keychain: bool) -> Vec<RawConnection> {
    let group_names: HashMap<String, String> = groups
        .iter()
        .filter_map(Value::as_object)
        .filter_map(|group| Some((text(group, &["id"])?, text(group, &["name"])?)))
        .collect();

    connections
        .iter()
        .filter_map(Value::as_object)
        .filter_map(|conn| {
            let name = text(conn, &["connectionname", "name"])?;
            let driver = canonical_driver(
                &text(conn, &["driver", "dbdriver", "databasetype", "type"]).unwrap_or_default(),
            );
            let database = if driver == "SQLite" {
                text(
                    conn,
                    &["databasepath", "dbpath", "path", "databasename", "dbname"],
                )
            } else {
                text(conn, &["databasename", "dbname", "database"])
            };

            let mut raw = RawConnection {
                host: text(conn, &["databasehost", "dbhost", "host"]).unwrap_or_default(),
                port: field(conn, &["databaseport", "dbport", "port"]).and_then(port),
                username: text(conn, &["databaseuser", "dbuser", "user", "username"])
                    .unwrap_or_default(),
                database,
                group: text(conn, &["groupid"])
                    .and_then(|id| group_names.get(&id).cloned())
                    .or_else(|| text(conn, &["groupname", "group"])),
                ..RawConnection::new(name, driver)
            };

            // Conexão salva só como URL (`db-url`): os campos vêm dela
            if raw.host.is_empty() && raw.driver != "SQLite" {
                if let Some(from_url) = text(conn, &["dburl", "url"])
                    .as_deref()
                    .and_then(url::connection_from_url)
                {
                    raw.host = from_url.host;
                    raw.port = raw.port.or(from_url.port);
                    if raw.username.is_empty() {
                        raw.username = from_url.username;
                    }
                    raw.database = raw.database.or(from_url.database);
                    raw.uri = from_url.uri;
                    raw.ssl_mode = from_url.ssl_mode;
                    raw.secret = from_url.secret;
                }
            }

            // SSH: o plist tem a flag `isOverSSH`; o JSON do Linux não tem
            // flag, e o túnel está ligado quando `ssh-host` vem preenchido
            let ssh_host = text(conn, &["serveraddress", "sshhost"]);
            let over_ssh = match field(conn, &["isoverssh", "overssh", "usessh"]) {
                Some(flag) => truthy(flag),
                None => ssh_host.is_some(),
            };
            if over_ssh {
                raw.ssh_host = Some(ssh_host.unwrap_or_default());
            }
            // tLSMode / ssl-mode: -1 (Linux) ou 0 = sem exigência, 1 require,
            // 2 verify-ca, 3 verify-full
            raw.ssl_mode = match field(conn, &["tlsmode", "sslmode"]).and_then(Value::as_i64) {
                Some(1) => Some("require".into()),
                Some(2) => Some("verify-ca".into()),
                Some(3) => Some("verify-full".into()),
                _ => None,
            };
            // TlsKeyPaths: [CA, certificado do cliente, chave do cliente]; no
            // Linux, `ssl-key-path-1/2/3` na mesma ordem
            raw.client_certificate = field(conn, &["tlskeypaths"])
                .and_then(Value::as_array)
                .and_then(|paths| paths.get(1))
                .and_then(Value::as_str)
                .is_some_and(|path| !path.is_empty())
                || text(conn, &["sslkeypath2"]).is_some();

            // SQLite não tem senha: ler o Keychain só faria o macOS perguntar à toa
            if raw.driver != "SQLite" && raw.secret == Secret::None {
                if let Some(password) = text(conn, &["databasepassword", "password"]) {
                    raw.secret = Secret::Plain(password);
                } else if let Some(id) = text(conn, &["id"]).filter(|_| keychain) {
                    raw.secret = Secret::Keychain {
                        service: KEYCHAIN_SERVICE,
                        account: format!("{id}_database"),
                    };
                }
            }
            Some(raw)
        })
        .collect()
}

/// `ConnectionName` → `connectionname`; `ssl-mode` → `sslmode`.
fn normalize(key: &str) -> String {
    key.chars()
        .filter(|c| *c != '-' && *c != '_')
        .flat_map(char::to_lowercase)
        .collect()
}

/// O primeiro dos nomes (já normalizados) presente no objeto.
fn field<'a>(map: &'a Map<String, Value>, names: &[&str]) -> Option<&'a Value> {
    names.iter().find_map(|name| {
        map.iter()
            .find(|(key, _)| normalize(key) == *name)
            .map(|(_, value)| value)
    })
}

fn text(map: &Map<String, Value>, names: &[&str]) -> Option<String> {
    field(map, names)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// A porta vem como texto ou número.
fn port(value: &Value) -> Option<u16> {
    value
        .as_u64()
        .and_then(|n| u16::try_from(n).ok())
        .or_else(|| value.as_str()?.trim().parse().ok())
}

/// `true`, `1` ou `"true"`.
fn truthy(value: &Value) -> bool {
    match value {
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_i64().unwrap_or(0) != 0,
        Value::String(s) => matches!(s.trim(), "1" | "true" | "yes"),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("octapus-tableplus-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn reads_plist_style_keys_groups_and_keychain_refs() {
        let connections = vec![
            json!({
                "ID": "ABC", "ConnectionName": "Loja", "Driver": "PostgreSQL",
                "DatabaseHost": "db.loja.com", "DatabasePort": "5432",
                "DatabaseUser": "app", "DatabaseName": "loja", "GroupID": "G1",
                "isOverSSH": true, "ServerAddress": "bastion.loja.com", "tLSMode": 3
            }),
            json!({
                "ID": "DEF", "ConnectionName": "Notas", "Driver": "SQLite",
                "DatabasePath": "/Users/voce/notas.db"
            }),
            json!({ "ConnectionName": "Cache", "Driver": "Redis", "DatabasePort": 6380 }),
            // Sem nome: ignorada
            json!({ "Driver": "MySQL" }),
        ];
        let groups = vec![json!({ "ID": "G1", "Name": "Produção" })];

        let raws = parse(&connections, &groups, true);
        assert_eq!(raws.len(), 3);

        let pg = &raws[0];
        assert_eq!(pg.driver, "PostgreSQL");
        assert_eq!((pg.host.as_str(), pg.port), ("db.loja.com", Some(5432)));
        assert_eq!(pg.group.as_deref(), Some("Produção"));
        assert_eq!(pg.ssh_host.as_deref(), Some("bastion.loja.com"));
        assert_eq!(pg.ssl_mode.as_deref(), Some("verify-full"));
        assert_eq!(
            pg.secret,
            Secret::Keychain {
                service: KEYCHAIN_SERVICE,
                account: "ABC_database".into()
            }
        );

        assert_eq!(raws[1].database.as_deref(), Some("/Users/voce/notas.db"));
        assert_eq!(
            raws[1].secret,
            Secret::None,
            "SQLite não consulta o Keychain"
        );
        assert_eq!(raws[2].port, Some(6380));
        assert_eq!(raws[2].secret, Secret::None);

        // Fora do macOS não há Keychain para ler
        assert_eq!(parse(&connections, &groups, false)[0].secret, Secret::None);
    }

    /// Formato real do TablePlus no Linux (copiado de um `connections.json` de
    /// usuário, só com hosts, usuários e caminhos trocados)
    const LINUX_CONNECTIONS: &str = r#"{"objects":[
        {"id":"aaf7933c-6d4e-481f-b303-871cc2350052","name":"Loja DEV","color":"rgb(255,255,255)","tag":"development","db-driver":"PostgreSQL","db-url":"","db-host":"10.20.30.40","db-port":"","db-user":"postgres","db-name":"loja_dev","db-path":"","ssh-host":"","ssh-port":"","ssh-user":"","ssh-key-path":"","ssl-key-path-1":"","ssl-key-path-1-pass":"","ssl-key-path-2":"","ssl-key-path-3":"","selected-schema":"dev","favorites":[],"db-command-line-password":"","group-id":""},
        {"id":"f4136603-e5ce-4042-8745-8746ad1eb69c","name":"Loja LOCAL","color":"rgb(255,255,255)","tag":"local","db-driver":"PostgreSQL","db-url":"","db-host":"127.0.0.1","db-port":"5434","db-user":"voce","db-name":"loja","db-path":"","ssh-host":"","ssh-port":"","ssh-user":"","selected-schema":"cliente","favorites":[],"db-command-line-password":"","group-id":""},
        {"id":"c7b5b8ac-cb05-47ee-8671-ae0b688c1bde","name":"tarefas","color":"rgb(104,107,111)","tag":"","db-driver":"SQLite","db-url":"","db-host":"","db-port":"","db-user":"","db-name":"","db-path":"/home/voce/.local/share/tarefas/tarefas.db","ssh-host":"","ssh-port":"","ssh-user":"","ssl-mode":-1,"selected-schema":"tarefas.db","favorites":[],"db-command-line-password":"","group-id":"","recent-ai-chat-history":""},
        {"id":"x1","name":"Via túnel","db-driver":"PostgreSQL","db-host":"db.interno","db-port":"5432","db-user":"app","db-name":"app","ssh-host":"bastion.exemplo.com","ssh-port":"22","ssh-user":"deploy","ssl-mode":1,"ssl-key-path-2":"/home/voce/cert.pem"},
        {"id":"x2","name":"Só URL","db-driver":"PostgreSQL","db-url":"postgresql://app:s3cret@db.url.com:6543/vendas","db-host":"","db-port":""}
    ]}"#;

    #[test]
    fn reads_the_real_linux_format() {
        let dir = temp_dir("linux-real");
        let file = dir.join("connections.json");
        std::fs::write(&file, LINUX_CONNECTIONS).unwrap();

        let raws = read(&file).unwrap();
        assert_eq!(raws.len(), 5);

        let dev = &raws[0];
        assert_eq!(
            (dev.name.as_str(), dev.driver.as_str()),
            ("Loja DEV", "PostgreSQL")
        );
        assert_eq!(
            (dev.host.as_str(), dev.port),
            ("10.20.30.40", None),
            "porta vazia = padrão"
        );
        assert_eq!(dev.username, "postgres");
        assert_eq!(dev.database.as_deref(), Some("loja_dev"));
        assert_eq!(dev.ssh_host, None, "ssh-host vazio = sem túnel");
        assert_eq!(dev.ssl_mode, None);
        assert!(!dev.client_certificate);
        assert_eq!(dev.group, None);
        assert_eq!(
            dev.secret,
            Secret::None,
            "db-command-line-password não é senha"
        );

        assert_eq!(raws[1].port, Some(5434));

        let sqlite = &raws[2];
        assert_eq!(sqlite.driver, "SQLite");
        assert_eq!(
            sqlite.database.as_deref(),
            Some("/home/voce/.local/share/tarefas/tarefas.db")
        );

        let tunnel = &raws[3];
        assert_eq!(tunnel.ssh_host.as_deref(), Some("bastion.exemplo.com"));
        assert_eq!(tunnel.ssl_mode.as_deref(), Some("require"));
        assert!(tunnel.client_certificate);

        let from_url = &raws[4];
        assert_eq!(
            (from_url.host.as_str(), from_url.port),
            ("db.url.com", Some(6543))
        );
        assert_eq!(from_url.username, "app");
        assert_eq!(from_url.database.as_deref(), Some("vendas"));
        assert_eq!(from_url.secret, Secret::Plain("s3cret".into()));

        // Do arquivo até o candidato da prévia: Postgres na porta padrão, sem
        // senha (aviso), SQLite com o caminho na URI
        let candidates: Vec<_> = raws
            .iter()
            .enumerate()
            .map(|(key, raw)| {
                crate::services::connection_import::mapping::to_candidate(key, raw, &[])
            })
            .collect();
        assert_eq!(candidates[0].port, 5432);
        assert!(candidates[0]
            .warnings
            .contains(&crate::models::ImportWarning::NoPassword));
        assert_eq!(
            candidates[2].connection_uri.as_deref(),
            Some("/home/voce/.local/share/tarefas/tarefas.db")
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn reads_linux_json_with_kebab_case_keys() {
        let dir = temp_dir("linux");
        let file = dir.join("connections.json");
        std::fs::write(
            &file,
            json!([
                {
                    "id": "1", "connection-name": "ERP", "driver": "PostgreSQL",
                    "database-host": "db.erp", "database-port": 5433,
                    "database-user": "erp", "database-name": "erp",
                    "database-password": "s3cret", "ssl-mode": -1,
                    "is-over-ssh": 1, "server-address": "bastion.erp"
                },
                {
                    "id": "2", "connection-name": "Notas", "driver": "SQLite",
                    "database-path": "/home/voce/notas.db"
                }
            ])
            .to_string(),
        )
        .unwrap();

        let raws = read(&file).unwrap();
        assert_eq!(raws.len(), 2);
        let erp = &raws[0];
        assert_eq!(
            (erp.name.as_str(), erp.driver.as_str()),
            ("ERP", "PostgreSQL")
        );
        assert_eq!((erp.host.as_str(), erp.port), ("db.erp", Some(5433)));
        assert_eq!(erp.username, "erp");
        assert_eq!(erp.database.as_deref(), Some("erp"));
        assert_eq!(
            erp.ssl_mode, None,
            "-1 é o padrão do Linux: sem SSL exigido"
        );
        assert_eq!(erp.ssh_host.as_deref(), Some("bastion.erp"));
        assert_eq!(erp.secret, Secret::Plain("s3cret".into()));
        assert_eq!(raws[1].database.as_deref(), Some("/home/voce/notas.db"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn reads_wrapped_json_and_groups_in_the_same_file() {
        let dir = temp_dir("wrapped");
        let file = dir.join("connections.json");
        std::fs::write(
            &file,
            json!({
                "groups": [{ "id": "g", "name": "Clientes" }],
                "connections": [
                    { "name": "A", "type": "postgres", "host": "a", "group-id": "g" }
                ]
            })
            .to_string(),
        )
        .unwrap();
        let raws = read(&file).unwrap();
        assert_eq!(raws[0].driver, "PostgreSQL");
        assert_eq!(raws[0].group.as_deref(), Some("Clientes"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn reads_plist_file_and_rejects_garbage() {
        let dir = temp_dir("plist");
        let file = dir.join("Connections.plist");
        plist::Value::Array(vec![plist::Value::Dictionary(
            [
                (
                    "ConnectionName".to_string(),
                    plist::Value::String("X".into()),
                ),
                ("Driver".to_string(), plist::Value::String("Mongo".into())),
            ]
            .into_iter()
            .collect(),
        )])
        .to_file_xml(&file)
        .unwrap();
        assert_eq!(read(&file).unwrap()[0].driver, "MongoDB");

        let garbage = dir.join("lixo.json");
        std::fs::write(&garbage, "não é json").unwrap();
        assert!(read(&garbage).is_err());
        assert!(read(&dir.join("nao-existe.plist")).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
