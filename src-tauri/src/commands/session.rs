use tauri::State;

use crate::state::AppState;
use crate::storage::repositories::session;

/// Snapshot das abas salvo na última sessão (JSON gerado pelo front), ou `null`.
#[tauri::command]
pub fn load_session(state: State<'_, AppState>) -> Result<Option<String>, String> {
    session::load(&state.storage).map_err(|e| e.to_string())
}

/// Grava o snapshot das abas. É `async` para não ocupar a main thread: o front
/// chama isto enquanto o usuário digita no editor (com throttle) e serializa as
/// chamadas, então a ordem de chegada é garantida do lado de lá.
#[tauri::command]
pub async fn save_session(state: State<'_, AppState>, snapshot: String) -> Result<(), String> {
    session::save(&state.storage, &snapshot).map_err(|e| e.to_string())
}
