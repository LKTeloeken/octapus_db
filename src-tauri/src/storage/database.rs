use std::path::Path;

use rusqlite::{Connection, Result};

pub fn init_storage<P: AsRef<Path>>(db_path: P) -> Result<Connection> {
    let conn = Connection::open(db_path)?;

    conn.execute_batch(
        r#"
        PRAGMA journal_mode = WAL;
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS servers (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            name            TEXT NOT NULL,
            db_type         TEXT NOT NULL DEFAULT 'postgres',
            host            TEXT NOT NULL,
            port            INTEGER NOT NULL DEFAULT 5432,
            username        TEXT NOT NULL,
            password        TEXT NOT NULL,
            default_database TEXT,
            ssl_enabled     INTEGER NOT NULL DEFAULT 0,
            created_at      INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
        );

        CREATE INDEX IF NOT EXISTS idx_servers_name ON servers(name);
        "#,
    )?;

    Ok(conn)
}