use tauri::State;

use crate::models::{AdapterCapabilities, QueryResult, TableDataRequest};
use crate::state::AppState;
use crate::storage::repositories::servers;

use super::connect_adapter;

#[tauri::command]
pub async fn fetch_table_data(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
    request: TableDataRequest,
) -> Result<QueryResult, String> {
    let adapter = connect_adapter(&state, server_id, &database).await?;

    adapter
        .fetch_table_data(request)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_capabilities(
    state: State<'_, AppState>,
    server_id: i64,
) -> Result<AdapterCapabilities, String> {
    // Capabilities depend only on the db type — no connection, no keychain.
    let server = servers::get_by_id_meta(&state.storage, server_id).map_err(|e| e.to_string())?;

    AdapterCapabilities::for_db_type(server.db_type)
        .ok_or_else(|| format!("No adapter available for {:?}", server.db_type))
}
