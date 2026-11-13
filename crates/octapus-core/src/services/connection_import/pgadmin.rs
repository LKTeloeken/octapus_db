//! pgAdmin 4: o JSON de "Tools › Import/Export Servers" ou, direto, o
//! `pgadmin4.db` do modo desktop. Nenhum dos dois traz senha — o `.pgpass`
//! completa (ver `mapping::fill_from_pgpass`).

use std::collections::HashMap;
use std::path::Path;

use rusqlite::{Connection, OpenFlags};
use serde_json::Value;

use super::mapping::RawConnection;
use super::sqlite_util::{select_existing, table_exists};
use crate::error::{Error, Result};

/// Grupo que o pgAdmin cria sozinho e onde tudo cai por padrão — virar grupo
/// no Octapus seria só uma pasta a mais.
const DEFAULT_GROUP: &str = "Servers";

fn group(name: Option<String>) -> Option<String> {
    name.filter(|name| name != DEFAULT_GROUP)
}

/// `.json` é o export; qualquer outro arquivo, o banco do pgAdmin.
pub fn read(file: &Path) -> Result<Vec<RawConnection>> {
    let is_json = file
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("json"));
    if is_json {
        let text = std::fs::read_to_string(file).map_err(|e| {
            Error::InvalidState(format!("Não deu para ler {}: {e}", file.display()))
        })?;
        parse_json(&text)
    } else {
        read_db(file)
    }
}

pub fn parse_json(text: &str) -> Result<Vec<RawConnection>> {
    let json: Value = serde_json::from_str(text)
        .map_err(|e| Error::InvalidState(format!("JSON do pgAdmin inválido: {e}")))?;
    let servers = json
        .get("Servers")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            Error::InvalidState("O JSON não tem a chave \"Servers\" do pgAdmin".into())
        })?;

    Ok(servers
        .values()
        .filter_map(Value::as_object)
        .map(|server| {
            let text = |key: &str| {
                server
                    .get(key)
                    .and_then(Value::as_str)
                    .filter(|s| !s.is_empty())
                    .map(str::to_string)
            };
            let params = server.get("ConnectionParameters");
            let param = |key: &str| params.and_then(|p| p.get(key)).and_then(Value::as_str);

            RawConnection {
                host: text("Host")
                    .or_else(|| text("HostAddr"))
                    .unwrap_or_default(),
                port: server
                    .get("Port")
                    .and_then(Value::as_u64)
                    .and_then(|p| u16::try_from(p).ok()),
                username: text("Username").unwrap_or_default(),
                database: text("MaintenanceDB"),
                group: group(text("Group")),
                ssl_mode: param("sslmode")
                    .map(str::to_string)
                    .or_else(|| text("SSLMode")),
                client_certificate: param("sslcert").is_some(),
                ssh_host: (server.get("UseSSHTunnel").and_then(Value::as_i64) == Some(1))
                    .then(|| text("TunnelHost").unwrap_or_default()),
                ..RawConnection::new(text("Name").unwrap_or_default(), "PostgreSQL")
            }
        })
        .collect())
}

/// `pgadmin4.db`: tabelas `server` e `servergroup`. As colunas mudaram entre
/// versões (o `ssl_mode` foi para `connection_params` no 7) — lê o que houver.
pub fn read_db(file: &Path) -> Result<Vec<RawConnection>> {
    let conn = Connection::open_with_flags(file, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| Error::InvalidState(format!("Não deu para abrir {}: {e}", file.display())))?;
    if !table_exists(&conn, "server")? {
        return Err(Error::InvalidState(format!(
            "{} não é um banco do pgAdmin",
            file.display()
        )));
    }

    let groups: HashMap<i64, String> = if table_exists(&conn, "servergroup")? {
        let mut stmt = conn.prepare("SELECT id, name FROM servergroup")?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.filter_map(|row| row.ok()).collect()
    } else {
        HashMap::new()
    };

    let columns = [
        "name",
        "host",
        "port",
        "maintenance_db",
        "username",
        "servergroup_id",
        "use_ssh_tunnel",
        "tunnel_host",
        "ssl_mode",
        "connection_params",
    ];
    let sql = format!(
        "SELECT {} FROM server ORDER BY id",
        select_existing(&conn, "server", &columns)?
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |row| {
        let params: Option<Value> = row
            .get::<_, Option<String>>(9)?
            .and_then(|json| serde_json::from_str(&json).ok());
        let param = |key: &str| {
            params
                .as_ref()
                .and_then(|p| p.get(key))
                .and_then(Value::as_str)
                .map(str::to_string)
        };
        let group_id: Option<i64> = row.get(5)?;
        let ssh: Option<i64> = row.get(6)?;
        Ok(RawConnection {
            host: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
            port: row
                .get::<_, Option<i64>>(2)?
                .and_then(|p| u16::try_from(p).ok()),
            database: row.get(3)?,
            username: row.get::<_, Option<String>>(4)?.unwrap_or_default(),
            group: group(group_id.and_then(|id| groups.get(&id).cloned())),
            ssh_host: (ssh == Some(1)).then(|| {
                row.get::<_, Option<String>>(7)
                    .ok()
                    .flatten()
                    .unwrap_or_default()
            }),
            ssl_mode: param("sslmode").or(row.get::<_, Option<String>>(8)?),
            client_certificate: param("sslcert").is_some(),
            ..RawConnection::new(
                row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                "PostgreSQL",
            )
        })
    })?;
    Ok(rows.filter_map(|row| row.ok()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_export_json() {
        let raws = parse_json(
            r#"{ "Servers": {
                "1": { "Name": "Local", "Group": "Servers", "Host": "localhost", "Port": 5432,
                       "MaintenanceDB": "postgres", "Username": "postgres",
                       "ConnectionParameters": { "sslmode": "prefer" } },
                "2": { "Name": "Prod", "Group": "Clientes", "HostAddr": "10.0.0.5", "Port": 6432,
                       "MaintenanceDB": "erp", "Username": "erp", "SSLMode": "require",
                       "UseSSHTunnel": 1, "TunnelHost": "bastion" }
            } }"#,
        )
        .unwrap();
        assert_eq!(raws.len(), 2);
        let local = raws.iter().find(|r| r.name == "Local").unwrap();
        assert_eq!(
            local.group, None,
            "o grupo padrão do pgAdmin não vira grupo"
        );
        let prod = raws.iter().find(|r| r.name == "Prod").unwrap();
        assert_eq!((prod.host.as_str(), prod.port), ("10.0.0.5", Some(6432)));
        assert_eq!(prod.group.as_deref(), Some("Clientes"));
        assert_eq!(prod.ssl_mode.as_deref(), Some("require"));
        assert_eq!(prod.ssh_host.as_deref(), Some("bastion"));
        assert!(parse_json(r#"{"x":1}"#).is_err());
    }

    #[test]
    fn reads_desktop_db_with_new_and_old_columns() {
        let dir = std::env::temp_dir().join(format!("octapus-pgadmin-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("pgadmin4.db");
        let _ = std::fs::remove_file(&file);
        {
            let conn = Connection::open(&file).unwrap();
            conn.execute_batch(
                "CREATE TABLE servergroup (id INTEGER PRIMARY KEY, user_id INTEGER, name TEXT);
                 INSERT INTO servergroup VALUES (1, 1, 'Clientes');
                 CREATE TABLE server (id INTEGER PRIMARY KEY, name TEXT, host TEXT, port INTEGER,
                     maintenance_db TEXT, username TEXT, servergroup_id INTEGER,
                     use_ssh_tunnel INTEGER, tunnel_host TEXT, connection_params TEXT);
                 INSERT INTO server VALUES (1, 'ERP', 'db.erp', 5433, 'erp', 'app', 1, 0, NULL,
                     '{\"sslmode\": \"verify-ca\"}');",
            )
            .unwrap();
        }
        let raws = read(&file).unwrap();
        assert_eq!(raws.len(), 1);
        let erp = &raws[0];
        assert_eq!((erp.name.as_str(), erp.port), ("ERP", Some(5433)));
        assert_eq!(erp.group.as_deref(), Some("Clientes"));
        assert_eq!(erp.ssl_mode.as_deref(), Some("verify-ca"));
        assert_eq!(erp.ssh_host, None);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
