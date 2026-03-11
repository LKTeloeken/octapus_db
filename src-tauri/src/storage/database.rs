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

    run_migrations(&conn)?;

    Ok(conn)
}

fn run_migrations(conn: &Connection) -> Result<()> {
    let existing_columns: Vec<String> = {
        let mut stmt = conn.prepare("PRAGMA table_info(servers)")?;
        let cols = stmt.query_map([], |row| row.get::<_, String>(1))?;
        cols.filter_map(|c| c.ok()).collect()
    };

    let migrations: &[(&str, &str)] = &[
        ("db_type", "ALTER TABLE servers ADD COLUMN db_type TEXT NOT NULL DEFAULT 'postgres'"),
        ("ssl_enabled", "ALTER TABLE servers ADD COLUMN ssl_enabled INTEGER NOT NULL DEFAULT 0"),
    ];

    for (col_name, alter_sql) in migrations {
        if !existing_columns.iter().any(|c| c == col_name) {
            conn.execute(alter_sql, [])?;
        }
    }

    Ok(())
}