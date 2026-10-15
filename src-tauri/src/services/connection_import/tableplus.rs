//! TablePlus. Cada sistema guarda as conexões de um jeito:
//!
//! - macOS (e Windows): `Connections.plist` + `ConnectionGroups.plist` na pasta
//!   `Data`, com as senhas no Keychain (serviço `com.tableplus.TablePlus`,
//!   conta `<ID>_database`) — lidas só no commit, ver `keychain.rs`;
//! - Linux: `~/.tableplus/settings/connections.json`. O formato não é público:
//!   as chaves seguem o kebab-case do app (`"ssl-mode": -1`, como aparece nas
//!   issues do TablePlus-Linux).
//!
//! Os dois viram o mesmo `serde_json::Value` e cada campo é procurado pelo nome
//! normalizado (sem caixa, `-` e `_`): `ConnectionName`, `connection-name` e
//! `connection_name` são a mesma chave. Assim o JSON do Linux é lido com os
//! nomes que o plist já usa, e um nome diferente custa só um sinônimo aqui.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use super::mapping::{canonical_driver, RawConnection, Secret};
use super::paths;
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
    let document = load(file).ok_or_else(invalid)?;
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
            .and_then(|doc| doc.as_array().cloned())
        })
        .unwrap_or_default();

    Ok(parse(&connections, &groups, cfg!(target_os = "macos")))
}

/// plist (XML ou binário) ou JSON, pelo conteúdo — não pela extensão.
fn load(path: &Path) -> Option<Value> {
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok().or_else(|| {
        plist::Value::from_reader(std::io::Cursor::new(&bytes))
            .ok()
            .and_then(|value| serde_json::to_value(value).ok())
    })
}

/// A lista de conexões: o próprio documento ou uma lista dentro dele
/// (`{"connections": [...]}`).
fn connection_list(document: &Value) -> Option<Vec<Value>> {
    match document {
        Value::Array(items) => Some(items.clone()),
        Value::Object(map) => field(map, &["connections", "items"])?.as_array().cloned(),
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
                &text(conn, &["driver", "databasetype", "type"]).unwrap_or_default(),
            );
            let database = if driver == "SQLite" {
                text(conn, &["databasepath", "path", "databasename"])
            } else {
                text(conn, &["databasename", "database"])
            };

            let mut raw = RawConnection {
                host: text(conn, &["databasehost", "host"]).unwrap_or_default(),
                port: field(conn, &["databaseport", "port"]).and_then(port),
                username: text(conn, &["databaseuser", "user", "username"]).unwrap_or_default(),
                database,
                group: text(conn, &["groupid"])
                    .and_then(|id| group_names.get(&id).cloned())
                    .or_else(|| text(conn, &["groupname", "group"])),
                ..RawConnection::new(name, driver)
            };

            if field(conn, &["isoverssh", "overssh", "usessh"]).is_some_and(truthy) {
                raw.ssh_host = Some(text(conn, &["serveraddress", "sshhost"]).unwrap_or_default());
            }
            // tLSMode / ssl-mode: -1 (Linux) ou 0 = sem exigência, 1 require,
            // 2 verify-ca, 3 verify-full
            raw.ssl_mode = match field(conn, &["tlsmode", "sslmode"]).and_then(Value::as_i64) {
                Some(1) => Some("require".into()),
                Some(2) => Some("verify-ca".into()),
                Some(3) => Some("verify-full".into()),
                _ => None,
            };
            // TlsKeyPaths: [CA, certificado do cliente, chave do cliente]
            raw.client_certificate = field(conn, &["tlskeypaths"])
                .and_then(Value::as_array)
                .and_then(|paths| paths.get(1))
                .and_then(Value::as_str)
                .is_some_and(|path| !path.is_empty());

            // SQLite não tem senha: ler o Keychain só faria o macOS perguntar à toa
            if raw.driver != "SQLite" {
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
