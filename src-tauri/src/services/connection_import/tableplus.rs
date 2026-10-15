//! TablePlus: `Connections.plist` e `ConnectionGroups.plist` na pasta `Data`.
//! As senhas ficam no Keychain (serviço `com.tableplus.TablePlus`, conta
//! `<ID>_database`) — lidas só no commit, ver `keychain.rs`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use plist::{Dictionary, Value};

use super::mapping::{canonical_driver, RawConnection, Secret};
use super::paths;
use crate::error::{Error, Result};

pub const KEYCHAIN_SERVICE: &str = "com.tableplus.TablePlus";

/// O `Connections.plist` encontrado (versão normal antes da do Setapp).
pub fn find_file() -> Option<PathBuf> {
    paths::tableplus_data_dirs()
        .into_iter()
        .map(|dir| dir.join("Connections.plist"))
        .find(|path| path.is_file())
}

/// Lê o `Connections.plist` e, da mesma pasta, os nomes dos grupos.
pub fn read(file: &Path) -> Result<Vec<RawConnection>> {
    let connections = load_array(file).ok_or_else(|| {
        Error::InvalidState(format!("{} não é um plist do TablePlus", file.display()))
    })?;
    let groups = file
        .parent()
        .and_then(|dir| load_array(&dir.join("ConnectionGroups.plist")))
        .unwrap_or_default();
    Ok(parse(&connections, &groups))
}

fn load_array(path: &Path) -> Option<Vec<Value>> {
    Value::from_file(path).ok()?.into_array()
}

pub fn parse(connections: &[Value], groups: &[Value]) -> Vec<RawConnection> {
    let group_names: HashMap<&str, &str> = groups
        .iter()
        .filter_map(Value::as_dictionary)
        .filter_map(|group| Some((text(group, "ID")?, text(group, "Name")?)))
        .collect();

    connections
        .iter()
        .filter_map(Value::as_dictionary)
        .filter_map(|dict| {
            let name = text(dict, "ConnectionName")?;
            let driver = canonical_driver(text(dict, "Driver").unwrap_or_default());
            let database = if driver == "SQLite" {
                text(dict, "DatabasePath")
            } else {
                text(dict, "DatabaseName")
            };

            let mut raw = RawConnection {
                host: text(dict, "DatabaseHost").unwrap_or_default().to_string(),
                port: dict.get("DatabasePort").and_then(port),
                username: text(dict, "DatabaseUser").unwrap_or_default().to_string(),
                database: database.map(str::to_string),
                group: text(dict, "GroupID")
                    .and_then(|id| group_names.get(id))
                    .map(|name| name.to_string()),
                ..RawConnection::new(name, driver)
            };

            if dict.get("isOverSSH").and_then(Value::as_boolean) == Some(true) {
                raw.ssh_host = Some(text(dict, "ServerAddress").unwrap_or_default().to_string());
            }
            // tLSMode: 0 prefer, 1 require, 2 verify-ca, 3 verify-full
            raw.ssl_mode = match dict.get("tLSMode").and_then(Value::as_signed_integer) {
                Some(1) => Some("require".into()),
                Some(2) => Some("verify-ca".into()),
                Some(3) => Some("verify-full".into()),
                _ => None,
            };
            // TlsKeyPaths: [CA, certificado do cliente, chave do cliente]
            raw.client_certificate = dict
                .get("TlsKeyPaths")
                .and_then(Value::as_array)
                .and_then(|paths| paths.get(1))
                .and_then(Value::as_string)
                .is_some_and(|path| !path.is_empty());

            // SQLite não tem senha: ler o Keychain só faria o macOS perguntar à toa
            if let Some(id) = text(dict, "ID").filter(|_| raw.driver != "SQLite") {
                raw.secret = Secret::Keychain {
                    service: KEYCHAIN_SERVICE,
                    account: format!("{id}_database"),
                };
            }
            Some(raw)
        })
        .collect()
}

fn text<'a>(dict: &'a Dictionary, key: &str) -> Option<&'a str> {
    dict.get(key)
        .and_then(Value::as_string)
        .filter(|s| !s.is_empty())
}

/// O TablePlus grava a porta como texto ou número.
fn port(value: &Value) -> Option<u16> {
    value.as_string().and_then(|s| s.parse().ok()).or_else(|| {
        value
            .as_signed_integer()
            .and_then(|n| u16::try_from(n).ok())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dict(pairs: &[(&str, Value)]) -> Value {
        Value::Dictionary(
            pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
        )
    }

    fn s(value: &str) -> Value {
        Value::String(value.into())
    }

    #[test]
    fn reads_connections_groups_and_keychain_refs() {
        let connections = vec![
            dict(&[
                ("ID", s("ABC")),
                ("ConnectionName", s("Loja")),
                ("Driver", s("PostgreSQL")),
                ("DatabaseHost", s("db.loja.com")),
                ("DatabasePort", s("5432")),
                ("DatabaseUser", s("app")),
                ("DatabaseName", s("loja")),
                ("GroupID", s("G1")),
                ("isOverSSH", Value::Boolean(true)),
                ("ServerAddress", s("bastion.loja.com")),
                ("tLSMode", Value::Integer(3.into())),
            ]),
            dict(&[
                ("ID", s("DEF")),
                ("ConnectionName", s("Notas")),
                ("Driver", s("SQLite")),
                ("DatabasePath", s("/Users/voce/notas.db")),
            ]),
            dict(&[
                ("ConnectionName", s("Cache")),
                ("Driver", s("Redis")),
                ("DatabasePort", Value::Integer(6380.into())),
            ]),
            // Sem nome: ignorada
            dict(&[("Driver", s("MySQL"))]),
        ];
        let groups = vec![dict(&[("ID", s("G1")), ("Name", s("Produção"))])];

        let raws = parse(&connections, &groups);
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
        assert_eq!(raws[1].secret, Secret::None);
        assert_eq!(raws[2].port, Some(6380));
        assert_eq!(raws[2].secret, Secret::None);
    }

    #[test]
    fn reads_plist_file() {
        let dir = std::env::temp_dir().join(format!("octapus-tableplus-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("Connections.plist");
        Value::Array(vec![dict(&[
            ("ConnectionName", s("X")),
            ("Driver", s("Mongo")),
        ])])
        .to_file_xml(&file)
        .unwrap();
        let raws = read(&file).unwrap();
        assert_eq!(raws[0].driver, "MongoDB");
        assert!(read(&dir.join("nao-existe.plist")).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
