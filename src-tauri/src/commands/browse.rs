use tauri::State;

use crate::error::{Error, Result};
use crate::models::{AdapterCapabilities, DbTypeCapabilities, QueryResult, TableDataRequest};
use crate::state::AppState;
use crate::storage::repositories::servers;

use super::connect_adapter;

#[tauri::command]
pub async fn fetch_table_data(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
    request: TableDataRequest,
) -> Result<QueryResult> {
    let adapter = connect_adapter(&state, server_id, &database).await?;

    adapter.fetch_table_data(request).await
}

/// As capabilities de todos os tipos de banco, de uma vez: dependem só do
/// tipo, e o front decide por servidor (árvore, palette) sem uma consulta por
/// servidor. Estático — o front guarda sem prazo.
#[tauri::command]
pub fn get_db_capabilities() -> Vec<DbTypeCapabilities> {
    AdapterCapabilities::all()
}

#[tauri::command]
pub fn get_capabilities(state: State<'_, AppState>, server_id: i64) -> Result<AdapterCapabilities> {
    // Capabilities depend only on the db type — no connection, no keychain.
    let server = servers::get_by_id_meta(&state.storage, server_id)?;

    AdapterCapabilities::for_db_type(server.db_type).ok_or_else(|| {
        Error::UnsupportedDatabase(format!("no adapter available for {:?}", server.db_type))
    })
}
