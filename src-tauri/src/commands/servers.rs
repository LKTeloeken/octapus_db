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
    // Só o que muda a conexão ou o que se enxerga do banco derruba as conexões
    // e tira o catálogo da memória e do disco: renomear o servidor ou trocá-lo
    // de grupo não refaz nada.
    let current = servers::get_by_id_meta(&state.storage, id)?;
    if connection_changed(&current, &input) {
        state.connections.disconnect_server(id);
        state.catalog.forget_server(id);
    }

    servers::update(&state.storage, id, input)
}

/// A edição mexe em como o app conecta (endereço, credencial, TLS, fuso da
/// sessão) ou no escopo do catálogo? Senha informada conta como mudança: a
/// salva não é decifrada só para comparar.
fn connection_changed(current: &Server, input: &ServerInput) -> bool {
    fn blank_to_none(value: &Option<String>) -> Option<&str> {
        value.as_deref().filter(|text| !text.trim().is_empty())
    }

    let next = input.to_server(current.id);
    input.password.is_some()
        || next.db_type != current.db_type
        || next.host != current.host
        || next.port != current.port
        || next.username != current.username
        || next.default_database.as_deref() != blank_to_none(&current.default_database)
        || next.ssl_mode != current.ssl_mode
        || next.ssl_root_cert != current.ssl_root_cert
        || next.connection_uri.as_deref() != blank_to_none(&current.connection_uri)
        || next.display_timezone != current.display_timezone
        || blank_to_none(&input.scope_databases) != blank_to_none(&current.scope_databases)
        || blank_to_none(&input.scope_schemas) != blank_to_none(&current.scope_schemas)
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
