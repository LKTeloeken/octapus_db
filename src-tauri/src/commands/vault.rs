use serde::Serialize;
use tauri::State;

use crate::error::Error;
use crate::state::AppState;
use crate::storage::health::{self, VaultHealth};
use crate::storage::vault::{self, VaultState};

/// Estado do cofre, consultado pelo front no boot e depois de cada operação.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VaultStatus {
    /// Há senha mestre configurada nesta instalação.
    pub has_master_password: bool,
    /// Trancado: existe senha mestre e ela ainda não foi digitada nesta sessão.
    pub locked: bool,
    /// `false` quando a chave não abre o que está guardado (canário não bate)
    /// ou quando o `vault.key` está ilegível. Trancado **não** conta como
    /// não-saudável: ali a resposta ainda é desconhecida.
    pub healthy: bool,
    /// `vault.key` ilegível em qualquer formato — só resta o reset.
    pub corrupt: bool,
}

#[tauri::command]
pub fn vault_status(state: State<'_, AppState>) -> VaultStatus {
    let vault_state = vault::state();

    let healthy = match vault_state {
        VaultState::Unlocked => {
            health::check_or_seed(&state.storage.lock()) == VaultHealth::Ok
        }
        VaultState::Locked => true,
        VaultState::Corrupt => false,
    };

    VaultStatus {
        has_master_password: vault::has_master_password(),
        locked: vault_state == VaultState::Locked,
        healthy,
        corrupt: vault_state == VaultState::Corrupt,
    }
}

/// Destrava o cofre para a sessão. Senha errada não decifra (tag do GCM).
#[tauri::command]
pub fn vault_unlock(state: State<'_, AppState>, password: String) -> Result<(), String> {
    vault::unlock(&password).map_err(|e| e.to_string())?;

    // A senha estava certa, mas a chave pode ser de outra instalação: o canário
    // é quem distingue "senha correta" de "chave correta para *este* banco".
    if health::check_or_seed(&state.storage.lock()) != VaultHealth::Ok {
        vault::lock();
        return Err(Error::VaultUnavailable.to_string());
    }

    Ok(())
}

/// Tranca o cofre e derruba as conexões abertas — sem isso, os pools já criados
/// continuariam servindo dados e "trancado" não significaria nada.
#[tauri::command]
pub fn vault_lock(state: State<'_, AppState>) -> Result<(), String> {
    if !vault::has_master_password() {
        return Err(Error::InvalidState(
            "Não há senha mestre configurada para trancar".into(),
        )
        .to_string());
    }

    state.connections.disconnect_all();
    vault::lock();
    Ok(())
}

/// Ativa a senha mestre. Nenhum dado é re-criptografado: o mesmo DEK passa a ser
/// guardado embrulhado pela senha.
#[tauri::command]
pub fn vault_enable_master_password(password: String) -> Result<(), String> {
    vault::enable_master_password(&password).map_err(|e| e.to_string())
}

/// Desativa a senha mestre, voltando a chave crua para o disco. Exige a senha
/// atual mesmo com o cofre destravado — senão qualquer um numa máquina aberta
/// removeria a proteção sem saber o segredo.
#[tauri::command]
pub fn vault_disable_master_password(password: String) -> Result<(), String> {
    vault::unlock(&password).map_err(|e| e.to_string())?;
    vault::disable_master_password().map_err(|e| e.to_string())
}

/// **Destrutivo.** Único caminho para quem esqueceu a senha mestre: descarta a
/// chave, gera outra e apaga todo segredo guardado — que sem a chave antiga já
/// era irrecuperável de qualquer forma. Os servidores continuam cadastrados;
/// só as senhas e URIs precisam ser digitadas de novo.
#[tauri::command]
pub fn vault_reset(state: State<'_, AppState>) -> Result<(), String> {
    state.connections.disconnect_all();

    vault::reset().map_err(|e| e.to_string())?;

    let conn = state.storage.lock();
    conn.execute("UPDATE servers SET password = '', connection_uri = NULL", [])
        .map_err(|e| Error::Storage(e.to_string()).to_string())?;

    // O canário antigo foi selado com a chave descartada; sela um novo.
    health::clear_canary(&conn).map_err(|e| e.to_string())?;
    health::check_or_seed(&conn);

    Ok(())
}
