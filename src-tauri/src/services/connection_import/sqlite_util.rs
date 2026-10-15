//! Ler o SQLite de outro app sem depender da versão dele: coluna que não
//! existe vira `NULL`, e o índice de cada coluna no `SELECT` não muda.

use rusqlite::Connection;

use crate::error::Result;

pub fn table_exists(conn: &Connection, table: &str) -> Result<bool> {
    let count: i64 = conn.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?",
        [table],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

/// `"a", NULL AS "b", "c"` — as colunas pedidas, na ordem, as ausentes nulas.
pub fn select_existing(conn: &Connection, table: &str, columns: &[&str]) -> Result<String> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info(\"{table}\")"))?;
    let existing: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(|name| name.ok())
        .collect();
    Ok(columns
        .iter()
        .map(|column| {
            if existing.iter().any(|name| name == column) {
                format!("\"{column}\"")
            } else {
                format!("NULL AS \"{column}\"")
            }
        })
        .collect::<Vec<_>>()
        .join(", "))
}
