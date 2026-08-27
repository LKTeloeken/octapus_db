//! Chave/valor do próprio app (tabela `meta`). Guarda estado interno que não
//! pertence a nenhum servidor: o canário do cofre e a flag de `VACUUM` pendente.

use rusqlite::{params, Connection, OptionalExtension};

use crate::error::{Error, Result};

pub fn get(conn: &Connection, key: &str) -> Result<Option<String>> {
    conn.query_row("SELECT value FROM meta WHERE key = ?", [key], |row| {
        row.get::<_, String>(0)
    })
    .optional()
    .map_err(|e| Error::Storage(e.to_string()))
}

pub fn set(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO meta (key, value) VALUES (?1, ?2) \
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .map_err(|e| Error::Storage(e.to_string()))?;

    Ok(())
}

pub fn delete(conn: &Connection, key: &str) -> Result<()> {
    conn.execute("DELETE FROM meta WHERE key = ?", [key])
        .map_err(|e| Error::Storage(e.to_string()))?;

    Ok(())
}
