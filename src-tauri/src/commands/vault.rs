use serde::Serialize;
use tauri::State;

use crate::state::AppState;
use crate::storage::health::VaultHealth;

/// Estado do cofre, consultado pelo front no boot.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultStatus {
    /// `false` quando o `vault.key` atual não abre o que está guardado — as
    /// senhas salvas são irrecuperáveis e precisam ser cadastradas de novo.
    pub healthy: bool,
}

#[tauri::command]
pub fn vault_status(state: State<'_, AppState>) -> VaultStatus {
    VaultStatus {
        healthy: state.vault_health == VaultHealth::Ok,
    }
}
