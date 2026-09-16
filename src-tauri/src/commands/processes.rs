use tauri::State;

use crate::models::{DatabaseProcess, DatabaseType};
use crate::state::AppState;
use crate::storage::repositories::servers;

use super::connect_adapter;

fn ensure_postgres(state: &State<'_, AppState>, server_id: i64) -> Result<String, String> {
    let server = servers::get_by_id_meta(&state.storage, server_id).map_err(|e| e.to_string())?;

    if server.db_type != DatabaseType::Postgres {
        return Err("Process monitoring is only available for PostgreSQL servers".into());
    }

    Ok(server
        .default_database
        .unwrap_or_else(|| server.db_type.default_database_name().to_string()))
}

#[tauri::command]
pub async fn list_processes(
    state: State<'_, AppState>,
    server_id: i64,
) -> Result<Vec<DatabaseProcess>, String> {
    let database = ensure_postgres(&state, server_id)?;
    let adapter = connect_adapter(&state, server_id, &database).await?;

    adapter.list_processes().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn terminate_process(
    state: State<'_, AppState>,
    server_id: i64,
    pid: i32,
) -> Result<bool, String> {
    let database = ensure_postgres(&state, server_id)?;
    let adapter = connect_adapter(&state, server_id, &database).await?;

    adapter
        .terminate_process(pid)
        .await
        .map_err(|e| e.to_string())
}
