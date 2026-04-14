use std::time::{SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;
use rusqlite::{params, Connection};

use crate::error::{Error, Result};
use crate::models::{DatabaseType, Server, ServerInput};

/// Get all servers
pub fn get_all(storage: &Mutex<Connection>) -> Result<Vec<Server>> {
    let conn = storage.lock();

    let mut stmt = conn.prepare(
        r#"
        SELECT id, name, db_type, host, port, username, password,
               default_database, ssl_enabled, created_at
        FROM servers
        ORDER BY name ASC
        "#,
    )?;

    let rows = stmt.query_map([], |row| {
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
            created_at: row.get(9)?,
        })
    })?;

    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::Storage(e.to_string()))
}

/// Get a server by ID
pub fn get_by_id(storage: &Mutex<Connection>, id: i64) -> Result<Server> {
    let conn = storage.lock();

    let mut stmt = conn.prepare(
        r#"
        SELECT id, name, db_type, host, port, username, password,
               default_database, ssl_enabled, created_at
        FROM servers
        WHERE id = ?
        "#,
    )?;

    stmt.query_row([id], |row| {
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
            created_at: row.get(9)?,
        })
    })
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            Error::NotFound(format!("Server with id {} not found", id))
        }
        _ => Error::Storage(e.to_string()),
    })
}

/// Create a new server
pub fn create(storage: &Mutex<Connection>, input: ServerInput) -> Result<Server> {
    let conn = storage.lock();

    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| Error::Storage(e.to_string()))?
        .as_secs() as i64;

    let db_type_str = db_type_to_string(&input.db_type);
    let ssl_enabled = input.ssl_enabled.unwrap_or(false) as i32;

    let mut stmt = conn.prepare(
        r#"
        INSERT INTO servers (name, db_type, host, port, username, password,
                            default_database, ssl_enabled, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        RETURNING id, name, db_type, host, port, username, password,
                  default_database, ssl_enabled, created_at
        "#,
    )?;

    stmt.query_row(
        params![
            input.name,
            db_type_str,
            input.host,
            input.port,
            input.username,
            input.password,
            input.default_database,
            ssl_enabled,
            created_at,
        ],
        |row| {
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
                created_at: row.get(9)?,
            })
        },
    )
    .map_err(|e| Error::Storage(e.to_string()))
}

/// Update an existing server
pub fn update(storage: &Mutex<Connection>, id: i64, input: ServerInput) -> Result<Server> {
    let conn = storage.lock();

    let db_type_str = db_type_to_string(&input.db_type);
    let ssl_enabled = input.ssl_enabled.unwrap_or(false) as i32;

    let mut stmt = conn.prepare(
        r#"
        UPDATE servers
        SET name = ?1, db_type = ?2, host = ?3, port = ?4, username = ?5,
            password = CASE WHEN ?6 = '' THEN password ELSE ?6 END,
            default_database = ?7, ssl_enabled = ?8
        WHERE id = ?9
        RETURNING id, name, db_type, host, port, username, password,
                  default_database, ssl_enabled, created_at
        "#,
    )?;

    stmt.query_row(
        params![
            input.name,
            db_type_str,
            input.host,
            input.port,
            input.username,
            input.password,
            input.default_database,
            ssl_enabled,
            id,
        ],
        |row| {
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
                created_at: row.get(9)?,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            Error::NotFound(format!("Server with id {} not found", id))
        }
        _ => Error::Storage(e.to_string()),
    })
}

/// Delete a server
pub fn delete(storage: &Mutex<Connection>, id: i64) -> Result<()> {
    let conn = storage.lock();

    let affected = conn.execute("DELETE FROM servers WHERE id = ?", [id])?;

    if affected == 0 {
        return Err(Error::NotFound(format!("Server with id {} not found", id)));
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn parse_db_type(s: String) -> DatabaseType {
    match s.to_lowercase().as_str() {
        "mysql" => DatabaseType::Mysql,
        "sqlite" => DatabaseType::Sqlite,
        _ => DatabaseType::Postgres,
    }
}

fn db_type_to_string(db_type: &DatabaseType) -> &'static str {
    match db_type {
        DatabaseType::Postgres => "postgres",
        DatabaseType::Mysql => "mysql",
        DatabaseType::Sqlite => "sqlite",
    }
}
