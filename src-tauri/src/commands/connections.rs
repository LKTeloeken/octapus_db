use std::time::Duration;

use tauri::State;

use crate::adapters::{self, PoolStats};
use crate::error::{Error, Result};
use crate::models::ServerInput;
use crate::state::AppState;
use crate::storage::repositories::servers;

use super::connect_adapter;

#[tauri::command]
pub async fn connect(
    state: State<'_, AppState>,
    server_id: i64,
    database: Option<String>,
) -> Result<bool> {
    // Resolve the target database from metadata only; the password is fetched
    // by connect_adapter, and only when the pool actually needs creating.
    let server = servers::get_by_id_meta(&state.storage, server_id)?;

    let db = database
        .or(server.default_database.clone())
        .unwrap_or_else(|| server.db_type.default_database_name().to_string());

    let adapter = connect_adapter(&state, server_id, &db).await?;

    // Verify connection works
    adapter.test_connection().await?;

    Ok(true)
}

#[tauri::command]
pub async fn test_connection(state: State<'_, AppState>, server_id: i64) -> Result<bool> {
    let server = servers::get_by_id_meta(&state.storage, server_id)?;

    let db = server
        .default_database
        .clone()
        .unwrap_or_else(|| server.db_type.default_database_name().to_string());

    let adapter = connect_adapter(&state, server_id, &db).await?;

    adapter.test_connection().await?;

    Ok(true)
}

/// Teto do "Testar conexão": um host que não responde não segura o diálogo.
const TEST_TIMEOUT: Duration = Duration::from_secs(20);

/// Testa os dados do formulário do servidor sem gravar: conecta fora do pool
/// do app, faz o ping e descarta a conexão. Na edição (`server_id`), a senha
/// em branco é a salva — decifrada só aqui, no clique. O erro volta inteiro
/// (código e mensagem com as causas), para o formulário mostrar o motivo.
#[tauri::command]
pub async fn test_server_input(
    state: State<'_, AppState>,
    input: ServerInput,
    server_id: Option<i64>,
) -> Result<()> {
    let mut server = input.to_server(server_id);
    if server.password.is_empty() {
        if let Some(id) = server_id {
            server.password = servers::get_by_id(&state.storage, id)?.password;
        }
    }
    let result = test_server(&server, TEST_TIMEOUT).await;
    match &result {
        Ok(()) => log::info!("teste de conexão ({:?}): ok", server.db_type),
        Err(error) => log::info!("teste de conexão ({:?}): falhou ({})", server.db_type, error.code()),
    }
    result
}

/// Conecta ao database padrão do servidor (ou o do tipo), faz o ping e solta
/// a conexão, em até `timeout`.
async fn test_server(server: &crate::models::Server, timeout: Duration) -> Result<()> {
    let database = server
        .default_database
        .clone()
        .unwrap_or_else(|| server.db_type.default_database_name().to_string());
    let test = async {
        let adapter = adapters::create_adapter(server, &database).await?;
        adapter.test_connection().await
    };
    match tokio::time::timeout(timeout, test).await {
        Ok(result) => result,
        Err(_) => Err(Error::ConnectionTimeout),
    }
}

#[tauri::command]
pub fn disconnect(
    state: State<'_, AppState>,
    server_id: i64,
    database: Option<String>,
) -> Result<()> {
    match database {
        Some(db) => state.connections.disconnect(server_id, &db),
        None => state.connections.disconnect_server(server_id),
    }
    Ok(())
}

#[tauri::command]
pub fn get_pool_stats(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
) -> Result<Option<PoolStats>> {
    Ok(state.connections.pool_stats(server_id, &database))
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{DatabaseType, SslMode};

    fn input(db_type: DatabaseType, port: u16) -> ServerInput {
        ServerInput {
            name: "teste".into(),
            db_type,
            host: "127.0.0.1".into(),
            port,
            username: "u".into(),
            password: "p".into(),
            default_database: None,
            ssl_mode: Some(SslMode::Disable),
            ssl_root_cert: None,
            connection_uri: None,
            scope_databases: None,
            scope_schemas: None,
            group_id: None,
            assistant_sample_values: false,
        }
    }

    /// Porta 1 em localhost: ninguém escuta, o sistema recusa na hora. O erro
    /// é de conexão e traz o motivo do sistema, não só "error connecting".
    #[tokio::test]
    async fn refused_connection_comes_back_with_the_reason() {
        let error = test_server(&input(DatabaseType::Postgres, 1).to_server(None), TEST_TIMEOUT)
            .await
            .unwrap_err();
        assert_eq!(error.code(), "CONNECTION_ERROR", "{error}");
        assert!(error.to_string().to_lowercase().contains("refused"), "{error}");

        let error = test_server(&input(DatabaseType::Redis, 1).to_server(None), TEST_TIMEOUT)
            .await
            .unwrap_err();
        assert_eq!(error.code(), "CONNECTION_ERROR", "{error}");
    }

    /// SQLite: arquivo que não existe não vira um banco vazio novo
    #[tokio::test]
    async fn missing_sqlite_file_fails() {
        let path = std::env::temp_dir().join("octapus-nao-existe/arquivo.db");
        let server = ServerInput {
            connection_uri: Some(path.to_string_lossy().into_owned()),
            ..input(DatabaseType::Sqlite, 0)
        };
        assert!(test_server(&server.to_server(None), TEST_TIMEOUT).await.is_err());
    }
}
