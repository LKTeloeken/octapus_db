use tauri::State;

use crate::error::Result;
use crate::models::{Server, ServerGroup, ServerInput};
use crate::state::AppState;
use crate::storage::repositories::{server_groups, servers};

#[tauri::command]
pub fn get_all_servers(state: State<'_, AppState>) -> Result<Vec<Server>> {
    servers::get_all(&state.storage)
}

#[tauri::command]
pub fn get_server(state: State<'_, AppState>, id: i64) -> Result<Server> {
    servers::get_by_id(&state.storage, id)
}

#[tauri::command]
pub fn create_server(state: State<'_, AppState>, input: ServerInput) -> Result<Server> {
    servers::create(&state.storage, input)
}

#[tauri::command]
pub fn update_server(state: State<'_, AppState>, id: i64, input: ServerInput) -> Result<Server> {
    // Disconnect existing connections when server config changes
    state.connections.disconnect_server(id);
    // O que se enxerga do banco pode ter mudado: o catálogo sai da memória e do disco
    state.catalog.forget_server(id);

    servers::update(&state.storage, id, input)
}

#[tauri::command]
pub fn delete_server(state: State<'_, AppState>, id: i64) -> Result<()> {
    // Disconnect all connections for this server first
    state.connections.disconnect_server(id);
    state.catalog.forget_server(id);

    servers::delete(&state.storage, id)
}
/// Move o servidor para um grupo (`null` = fora de grupo). Não mexe na
/// conexão aberta nem no catálogo: o grupo é só organização da sidebar.
#[tauri::command]
pub fn set_server_group(state: State<'_, AppState>, id: i64, group_id: Option<i64>) -> Result<()> {
    servers::set_group(&state.storage, id, group_id)
}

// ─────────────────────────────────────────────────────────────────────────────
// Grupos
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_all_server_groups(state: State<'_, AppState>) -> Result<Vec<ServerGroup>> {
    server_groups::get_all(&state.storage)
}

#[tauri::command]
pub fn create_server_group(state: State<'_, AppState>, name: String) -> Result<ServerGroup> {
    server_groups::create(&state.storage, &name)
}

#[tauri::command]
pub fn rename_server_group(
    state: State<'_, AppState>,
    id: i64,
    name: String,
) -> Result<ServerGroup> {
    server_groups::rename(&state.storage, id, &name)
}

/// Os servidores do grupo ficam fora de grupo — não são excluídos.
#[tauri::command]
pub fn delete_server_group(state: State<'_, AppState>, id: i64) -> Result<()> {
    server_groups::delete(&state.storage, id)
}
