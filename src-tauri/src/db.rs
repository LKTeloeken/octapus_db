use rusqlite::{Connection, Result};

pub fn init_database(db_path: &str) -> Result<Connection> {
    let conn = Connection::open(db_path)?;

    // Tabela de categorias
    conn.execute(
        "CREATE TABLE IF NOT EXISTS servers (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT    NOT NULL,
            host        TEXT    NOT NULL,
            port        INTEGER NOT NULL DEFAULT 5432,
            username    TEXT,
            password    TEXT,
            created_at  INTEGER NOT NULL DEFAULT (strftime('%s','now'))
        );",
        [],
    )?;

    Ok(conn)
}