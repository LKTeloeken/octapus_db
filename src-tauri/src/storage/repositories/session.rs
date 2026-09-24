use std::time::{SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};

use crate::error::{Error, Result};

/// Snapshot da sessão do workspace (abas abertas), ou `None` se nunca foi salvo.
///
/// O conteúdo é opaco para o backend: quem define o formato (e versiona) é o
/// front. As abas são estado de UI — aqui só garantimos que ele sobreviva a um
/// restart ou a um update do app.
pub fn load(storage: &Mutex<Connection>) -> Result<Option<String>> {
    let conn = storage.lock();

    conn.query_row(
        "SELECT snapshot FROM workspace_session WHERE id = 1",
        [],
        |row| row.get(0),
    )
    .optional()
    .map_err(Error::from)
}

/// Substitui o snapshot salvo. A escrita é uma transação do SQLite, então quando
/// o comando responde o dado já está em disco — é isso que deixa o front gravar
/// e reiniciar o app logo em seguida (update) sem perder nada.
pub fn save(storage: &Mutex<Connection>, snapshot: &str) -> Result<()> {
    let conn = storage.lock();

    let updated_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| Error::Storage(e.to_string()))?
        .as_secs() as i64;

    conn.execute(
        "INSERT INTO workspace_session (id, snapshot, updated_at) VALUES (1, ?1, ?2)
         ON CONFLICT(id) DO UPDATE SET snapshot = excluded.snapshot,
                                       updated_at = excluded.updated_at",
        params![snapshot, updated_at],
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::init_storage;

    fn memory_storage() -> Mutex<Connection> {
        Mutex::new(init_storage(":memory:").expect("storage em memória"))
    }

    #[test]
    fn load_without_snapshot_returns_none() {
        let storage = memory_storage();
        assert_eq!(load(&storage).unwrap(), None);
    }

    #[test]
    fn save_overwrites_previous_snapshot() {
        let storage = memory_storage();

        save(&storage, r#"{"version":1,"tabs":[]}"#).unwrap();
        save(&storage, r#"{"version":1,"tabs":[{"id":"a"}]}"#).unwrap();

        assert_eq!(
            load(&storage).unwrap().as_deref(),
            Some(r#"{"version":1,"tabs":[{"id":"a"}]}"#),
        );

        let rows: i64 = storage
            .lock()
            .query_row("SELECT COUNT(*) FROM workspace_session", [], |row| row.get(0))
            .unwrap();
        assert_eq!(rows, 1);
    }
}
