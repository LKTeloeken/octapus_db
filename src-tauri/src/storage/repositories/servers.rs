use std::time::{SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;
use rusqlite::{params, Connection, Row};

use crate::error::{Error, Result};
use crate::models::{DatabaseType, Server, ServerInput};
use crate::storage::{secrets, vault};

const SELECT_COLUMNS: &str = "id, name, db_type, host, port, username, password, \
                              default_database, ssl_enabled, connection_uri, created_at";

/// Get all servers (metadata only). The UI never displays passwords, so the
/// stored ciphertext is never decrypted nor returned here.
pub fn get_all(storage: &Mutex<Connection>) -> Result<Vec<Server>> {
    let conn = storage.lock();

    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLUMNS} FROM servers ORDER BY name ASC"
    ))?;

    let rows = stmt.query_map([], map_row)?;
    let mut servers = rows
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::Storage(e.to_string()))?;

    // Never expose the stored ciphertext.
    for server in &mut servers {
        server.password.clear();
    }

    Ok(servers)
}

/// Get a server by ID with its **decrypted** password (needed to open a pool).
/// Prefer [`get_by_id_meta`] when only metadata is required.
pub fn get_by_id(storage: &Mutex<Connection>, id: i64) -> Result<Server> {
    let conn = storage.lock();
    let mut server = select_one(&conn, id)?;
    decrypt_password(&conn, &mut server);
    Ok(server)
}

/// Get a server by ID without decrypting the password — use it for metadata-only
/// needs (db type, default database). The `password` field comes back empty.
pub fn get_by_id_meta(storage: &Mutex<Connection>, id: i64) -> Result<Server> {
    let conn = storage.lock();
    let mut server = select_one(&conn, id)?;
    server.password.clear();
    Ok(server)
}

fn select_one(conn: &Connection, id: i64) -> Result<Server> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLUMNS} FROM servers WHERE id = ?"
    ))?;

    stmt.query_row([id], map_row).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            Error::NotFound(format!("Server with id {} not found", id))
        }
        _ => Error::Storage(e.to_string()),
    })
}

/// Create a new server. The password is encrypted into the vault before being
/// stored — the SQLite column only ever holds ciphertext.
pub fn create(storage: &Mutex<Connection>, input: ServerInput) -> Result<Server> {
    let conn = storage.lock();

    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| Error::Storage(e.to_string()))?
        .as_secs() as i64;

    let db_type_str = db_type_to_string(&input.db_type);
    let ssl_enabled = input.ssl_enabled.unwrap_or(false) as i32;
    let encrypted = vault::encrypt(&input.password)?;

    let mut stmt = conn.prepare(&format!(
        "INSERT INTO servers (name, db_type, host, port, username, password, \
                              default_database, ssl_enabled, connection_uri, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10) \
         RETURNING {SELECT_COLUMNS}"
    ))?;

    let mut server = stmt
        .query_row(
            params![
                input.name,
                db_type_str,
                input.host,
                input.port,
                input.username,
                encrypted,
                input.default_database,
                ssl_enabled,
                input.connection_uri,
                created_at,
            ],
            map_row,
        )
        .map_err(|e| Error::Storage(e.to_string()))?;

    // Keep the plaintext in the returned value for immediate use.
    server.password = input.password;

    Ok(server)
}

/// Update an existing server. The password is encrypted into the vault before
/// being stored.
pub fn update(storage: &Mutex<Connection>, id: i64, input: ServerInput) -> Result<Server> {
    let conn = storage.lock();

    let db_type_str = db_type_to_string(&input.db_type);
    let ssl_enabled = input.ssl_enabled.unwrap_or(false) as i32;
    let encrypted = vault::encrypt(&input.password)?;

    let mut stmt = conn.prepare(&format!(
        "UPDATE servers \
         SET name = ?1, db_type = ?2, host = ?3, port = ?4, username = ?5, \
             password = ?6, default_database = ?7, ssl_enabled = ?8, connection_uri = ?9 \
         WHERE id = ?10 \
         RETURNING {SELECT_COLUMNS}"
    ))?;

    let mut server = stmt
        .query_row(
            params![
                input.name,
                db_type_str,
                input.host,
                input.port,
                input.username,
                encrypted,
                input.default_database,
                ssl_enabled,
                input.connection_uri,
                id,
            ],
            map_row,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                Error::NotFound(format!("Server with id {} not found", id))
            }
            _ => Error::Storage(e.to_string()),
        })?;

    server.password = input.password;

    Ok(server)
}

/// Delete a server. The encrypted secret lives in the row, so it goes away with
/// it; we also drop any legacy keychain entry best-effort.
pub fn delete(storage: &Mutex<Connection>, id: i64) -> Result<()> {
    let conn = storage.lock();

    let affected = conn.execute("DELETE FROM servers WHERE id = ?", [id])?;

    if affected == 0 {
        return Err(Error::NotFound(format!("Server with id {} not found", id)));
    }

    secrets::delete_password(id);

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn map_row(row: &Row<'_>) -> rusqlite::Result<Server> {
    Ok(Server {
        id: Some(row.get(0)?),
        name: row.get(1)?,
        db_type: parse_db_type(row.get::<_, String>(2)?),
        host: row.get(3)?,
        port: row.get(4)?,
        username: row.get(5)?,
        password: row.get(6)?,
        default_database: row.get(7)?,
        ssl_enabled: row.get::<_, i32>(8)? != 0,
        connection_uri: row.get(9)?,
        created_at: row.get(10)?,
    })
}

/// Replace `server.password` (the stored value) with the plaintext password.
///
/// - vault envelope → decrypt;
/// - empty column → legacy keychain install: read it once (last OS prompt),
///   re-encrypt into the vault and drop the keychain entry;
/// - plaintext column (pre-keychain install) → encrypt into the vault in place.
fn decrypt_password(conn: &Connection, server: &mut Server) {
    let Some(id) = server.id else { return };
    let stored = std::mem::take(&mut server.password);

    if vault::is_envelope(&stored) {
        if let Some(plain) = vault::decrypt(&stored) {
            server.password = plain;
        }
        return;
    }

    // Legacy value: keychain (empty column) or plaintext column.
    let plaintext = if stored.is_empty() {
        secrets::get_password(id)
    } else {
        Some(stored)
    };

    let Some(plain) = plaintext else { return };

    // Migrate into the vault so future reads never touch the keychain again.
    if let Ok(envelope) = vault::encrypt(&plain) {
        let _ = conn.execute(
            "UPDATE servers SET password = ?1 WHERE id = ?2",
            params![envelope, id],
        );
        secrets::delete_password(id);
    }

    server.password = plain;
}

fn parse_db_type(s: String) -> DatabaseType {
    match s.to_lowercase().as_str() {
        "mysql" => DatabaseType::Mysql,
        "sqlite" => DatabaseType::Sqlite,
        "mongodb" => DatabaseType::Mongodb,
        "redis" => DatabaseType::Redis,
        _ => DatabaseType::Postgres,
    }
}

fn db_type_to_string(db_type: &DatabaseType) -> &'static str {
    match db_type {
        DatabaseType::Postgres => "postgres",
        DatabaseType::Mysql => "mysql",
        DatabaseType::Sqlite => "sqlite",
        DatabaseType::Mongodb => "mongodb",
        DatabaseType::Redis => "redis",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::database::init_storage;

    fn sample_input() -> ServerInput {
        ServerInput {
            name: "vault-test".into(),
            db_type: DatabaseType::Postgres,
            host: "localhost".into(),
            port: 5432,
            username: "user".into(),
            password: "s3cret".into(),
            default_database: None,
            ssl_enabled: None,
            connection_uri: None,
        }
    }

    #[test]
    fn password_is_encrypted_at_rest() {
        vault::init_for_tests();
        let storage = Mutex::new(init_storage(":memory:").unwrap());

        let created = create(&storage, sample_input()).unwrap();
        let id = created.id.unwrap();

        // The returned value keeps the plaintext for immediate use.
        assert_eq!(created.password, "s3cret");

        // The SQLite column stores ciphertext, never the plaintext.
        let stored: String = storage
            .lock()
            .query_row("SELECT password FROM servers WHERE id = ?", [id], |r| {
                r.get(0)
            })
            .unwrap();
        assert_ne!(stored, "s3cret", "password leaked to SQLite column");
        assert!(vault::is_envelope(&stored), "expected a vault envelope");

        // Round-trip: get_by_id decrypts back to the plaintext.
        assert_eq!(get_by_id(&storage, id).unwrap().password, "s3cret");

        // Metadata paths never expose the secret.
        assert_eq!(get_by_id_meta(&storage, id).unwrap().password, "");
        assert_eq!(get_all(&storage).unwrap()[0].password, "");
    }
}
