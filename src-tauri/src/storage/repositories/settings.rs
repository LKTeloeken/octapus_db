use parking_lot::Mutex;
use rusqlite::{params, Connection};
use serde_json::{Map, Value};

use crate::error::{Error, Result};
use crate::models::AppSettings;

/// Preferências salvas, com o padrão no lugar do que nunca foi gravado.
pub fn load(storage: &Mutex<Connection>) -> Result<AppSettings> {
    let conn = storage.lock();

    let mut stmt = conn.prepare("SELECT key, value FROM app_settings")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;

    let mut fields = Map::new();
    for row in rows {
        let (key, value) = row?;
        // Valor ilegível (gravado à mão, versão antiga) não derruba o resto:
        // o campo volta ao padrão.
        if let Ok(value) = serde_json::from_str::<Value>(&value) {
            fields.insert(key, value);
        }
    }

    // Um campo com o tipo errado invalida o objeto inteiro no serde; nesse caso
    // vale o padrão de tudo em vez de um erro que travaria a tela.
    Ok(serde_json::from_value(Value::Object(fields)).unwrap_or_default())
}

/// Grava todas as preferências numa transação e devolve o que ficou salvo.
pub fn save(storage: &Mutex<Connection>, settings: &AppSettings) -> Result<AppSettings> {
    let Value::Object(fields) =
        serde_json::to_value(settings).map_err(|e| Error::Storage(e.to_string()))?
    else {
        return Err(Error::Storage("settings must serialize to an object".into()));
    };

    let mut conn = storage.lock();
    let tx = conn.transaction()?;
    for (key, value) in &fields {
        tx.execute(
            "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value.to_string()],
        )?;
    }
    tx.commit()?;

    Ok(settings.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::init_storage;

    fn memory_storage() -> Mutex<Connection> {
        Mutex::new(init_storage(":memory:").expect("storage em memória"))
    }

    #[test]
    fn load_without_rows_returns_defaults() {
        let storage = memory_storage();
        assert_eq!(load(&storage).unwrap(), AppSettings::default());
        assert!(!load(&storage).unwrap().beta_updates);
    }

    #[test]
    fn save_then_load_round_trips() {
        let storage = memory_storage();
        let settings = AppSettings { beta_updates: true };

        save(&storage, &settings).unwrap();
        assert_eq!(load(&storage).unwrap(), settings);

        save(&storage, &AppSettings { beta_updates: false }).unwrap();
        assert!(!load(&storage).unwrap().beta_updates);
    }

    #[test]
    fn unknown_and_invalid_rows_fall_back_to_defaults() {
        let storage = memory_storage();
        storage
            .lock()
            .execute_batch(
                "INSERT INTO app_settings (key, value) VALUES ('removedSetting', '1');
                 INSERT INTO app_settings (key, value) VALUES ('betaUpdates', 'not json');",
            )
            .unwrap();

        assert_eq!(load(&storage).unwrap(), AppSettings::default());
    }
}
