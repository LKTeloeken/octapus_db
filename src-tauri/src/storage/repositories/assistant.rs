use std::time::{SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};

use crate::error::{Error, Result};
use crate::storage::vault;

/// Grava o token do participante, cifrado pelo vault, junto com a URL da API
/// que o emitiu. Uma sessão só: entrar de novo substitui a anterior.
///
/// É credencial de acesso, não dado do assistente — o único registro dele em
/// disco (RN-01).
pub fn save_token(storage: &Mutex<Connection>, api_url: &str, token: &str) -> Result<()> {
    let encrypted = vault::encrypt(token)?;
    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| Error::Storage(e.to_string()))?
        .as_secs() as i64;

    storage.lock().execute(
        "INSERT INTO assistant_session (id, api_url, token, created_at) VALUES (1, ?1, ?2, ?3)
         ON CONFLICT(id) DO UPDATE SET api_url = excluded.api_url,
                                       token = excluded.token,
                                       created_at = excluded.created_at",
        params![api_url, encrypted, created_at],
    )?;
    Ok(())
}

/// O token salvo para esta API, decifrado. `None` sem sessão, com a sessão de
/// outra API (o token nunca vai a um host diferente do que o emitiu) ou com um
/// valor que o vault não abre (chave do dispositivo trocada).
pub fn load_token(storage: &Mutex<Connection>, api_url: &str) -> Result<Option<String>> {
    let row: Option<(String, String)> = storage
        .lock()
        .query_row(
            "SELECT api_url, token FROM assistant_session WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;

    let Some((saved_url, encrypted)) = row else {
        return Ok(None);
    };
    if saved_url != api_url {
        return Ok(None);
    }
    let token = vault::decrypt(&encrypted);
    if token.is_none() {
        log::warn!("assistente: token salvo não abre com a chave do vault; entre de novo");
    }
    Ok(token)
}

/// Sai: apaga o token local. A API não tem logout (o token vale até o convite
/// ser revogado), então isto só esquece a credencial nesta máquina.
pub fn clear(storage: &Mutex<Connection>) -> Result<()> {
    storage.lock().execute("DELETE FROM assistant_session", [])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::init_storage;

    const API: &str = "https://api.example.com";

    fn memory_storage() -> Mutex<Connection> {
        vault::init_for_tests();
        Mutex::new(init_storage(":memory:").expect("storage em memória"))
    }

    fn raw_token(storage: &Mutex<Connection>) -> String {
        storage
            .lock()
            .query_row("SELECT token FROM assistant_session", [], |row| row.get(0))
            .unwrap()
    }

    #[test]
    fn token_is_encrypted_at_rest_and_read_back() {
        let storage = memory_storage();
        assert_eq!(load_token(&storage, API).unwrap(), None);

        save_token(&storage, API, "tok_participante_123").unwrap();

        let raw = raw_token(&storage);
        assert!(vault::is_envelope(&raw));
        assert!(!raw.contains("tok_participante_123"));
        assert_eq!(
            load_token(&storage, API).unwrap().as_deref(),
            Some("tok_participante_123")
        );
    }

    #[test]
    fn new_session_replaces_the_previous_one() {
        let storage = memory_storage();
        save_token(&storage, API, "primeiro").unwrap();
        save_token(&storage, API, "segundo").unwrap();

        assert_eq!(load_token(&storage, API).unwrap().as_deref(), Some("segundo"));
        let rows: i64 = storage
            .lock()
            .query_row("SELECT COUNT(*) FROM assistant_session", [], |row| row.get(0))
            .unwrap();
        assert_eq!(rows, 1);
    }

    #[test]
    fn token_of_another_api_is_not_used() {
        let storage = memory_storage();
        save_token(&storage, API, "tok").unwrap();
        assert_eq!(load_token(&storage, "https://outra.example.com").unwrap(), None);
    }

    #[test]
    fn unreadable_token_is_treated_as_logged_out() {
        let storage = memory_storage();
        storage
            .lock()
            .execute(
                "INSERT INTO assistant_session (id, api_url, token, created_at)
                 VALUES (1, ?1, 'v1:00', 0)",
                params![API],
            )
            .unwrap();
        assert_eq!(load_token(&storage, API).unwrap(), None);
    }

    #[test]
    fn clear_forgets_the_token() {
        let storage = memory_storage();
        save_token(&storage, API, "tok").unwrap();
        clear(&storage).unwrap();
        assert_eq!(load_token(&storage, API).unwrap(), None);
        // Sair sem sessão não é erro
        clear(&storage).unwrap();
    }
}
