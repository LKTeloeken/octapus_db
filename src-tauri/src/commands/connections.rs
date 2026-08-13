use tauri::State;

use crate::adapters::PoolStats;
use crate::state::AppState;
use crate::storage::repositories::servers;

use super::connect_adapter;

#[tauri::command]
pub async fn connect(
    state: State<'_, AppState>,
    server_id: i64,
    database: Option<String>,
) -> Result<bool, String> {
    // Resolve the target database from metadata only; the password is fetched
    // by connect_adapter, and only when the pool actually needs creating.
    let server = servers::get_by_id_meta(&state.storage, server_id).map_err(|e| e.to_string())?;

    let db = database
        .or(server.default_database.clone())
        .unwrap_or_else(|| server.db_type.default_database_name().to_string());

    let adapter = connect_adapter(&state, server_id, &db).await?;

    // Verify connection works
    adapter
        .test_connection()
        .await
        .map_err(|e| e.to_string())?;

    Ok(true)
}

#[tauri::command]
pub async fn test_connection(
    state: State<'_, AppState>,
    server_id: i64,
) -> Result<bool, String> {
    let server = servers::get_by_id_meta(&state.storage, server_id).map_err(|e| e.to_string())?;

    let db = server
        .default_database
        .clone()
        .unwrap_or_else(|| server.db_type.default_database_name().to_string());

    let adapter = connect_adapter(&state, server_id, &db).await?;

    adapter
        .test_connection()
        .await
        .map_err(|e| e.to_string())?;

    Ok(true)
}

#[tauri::command]
pub fn disconnect(
    state: State<'_, AppState>,
    server_id: i64,
    database: Option<String>,
) -> Result<(), String> {
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
) -> Result<Option<PoolStats>, String> {
    Ok(state.connections.pool_stats(server_id, &database))
}