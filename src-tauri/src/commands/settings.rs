use tauri::State;

use crate::models::AppSettings;
use crate::state::AppState;
use crate::storage::repositories::settings as settings_store;

/// Preferências do app (padrões para o que nunca foi salvo).
#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    settings_store::load(&state.storage).map_err(|e| e.to_string())
}

/// Substitui as preferências e devolve o que ficou gravado.
#[tauri::command]
pub fn update_settings(
    state: State<'_, AppState>,
    settings: AppSettings,
) -> Result<AppSettings, String> {
    settings_store::save(&state.storage, &settings).map_err(|e| e.to_string())
}
