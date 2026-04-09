use tauri::State;

use crate::models::{Server, ServerInput};
use crate::state::AppState;
use crate::storage::repositories::servers;

#[tauri::command]
pub fn get_all_servers(state: State<'_, AppState>) -> Result<Vec<Server>, String> {
    servers::get_all(&state.storage).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_server(state: State<'_, AppState>, id: i64) -> Result<Server, String> {
    servers::get_by_id(&state.storage, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_server(state: State<'_, AppState>, input: ServerInput) -> Result<Server, String> {
    servers::create(&state.storage, input).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_server(
    state: State<'_, AppState>,
    id: i64,
    input: ServerInput,
) -> Result<Server, String> {
    // Disconnect existing connections when server config changes
    state.connections.disconnect_server(id);

    servers::update(&state.storage, id, input).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_server(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    // Disconnect all connections for this server first
    state.connections.disconnect_server(id);

    servers::delete(&state.storage, id).map_err(|e| e.to_string())
}