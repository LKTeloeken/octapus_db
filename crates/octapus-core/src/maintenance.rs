//! O laço de manutenção de um processo que fica aberto (app desktop, UI
//! nativa): catálogos vencidos saem do disco na partida; a cada minuto,
//! conexões ociosas fecham e o teto de memória do catálogo é respeitado.
//!
//! É o mesmo laço que o `src-tauri/src/lib.rs` roda hoje; o app Tauri pode
//! passar a chamar este depois. O terminal não usa: o processo dele é curto.

use std::sync::Arc;
use std::time::Duration;

use crate::state::AppState;

/// De quanto em quanto tempo o laço roda.
pub const MAINTENANCE_EVERY: Duration = Duration::from_secs(60);

/// Pools sem uso há mais que isto fecham (navegar por dezenas de databases de
/// tenant não pode acumular conexões no servidor do cliente)
pub const ADAPTER_IDLE_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// Roda para sempre: chame com `spawn` no runtime tokio do processo. O
/// catálogo já precisa estar iniciado (`state.catalog.init`).
pub async fn run(state: Arc<AppState>) {
    purge_expired(&state);

    let mut tick = tokio::time::interval(MAINTENANCE_EVERY);
    // O primeiro tick é imediato; a primeira rodada fica para daqui a um minuto
    tick.tick().await;
    loop {
        tick.tick().await;
        tick_once(&state).await;
    }
}

/// Apaga do disco os catálogos vencidos (na partida).
fn purge_expired(state: &AppState) {
    match state.catalog.purge_expired() {
        Ok(0) => {}
        Ok(purged) => log::info!("catálogo: {purged} arquivo(s) vencido(s) apagado(s)"),
        Err(error) => log::warn!("catálogo: falha ao apagar os vencidos: {error}"),
    }
}

/// Uma rodada: teto de memória do catálogo e conexões ociosas.
async fn tick_once(state: &AppState) {
    state.catalog.maintenance().await;
    let evicted = state.connections.evict_idle(ADAPTER_IDLE_TIMEOUT);
    if evicted > 0 {
        log::debug!("{evicted} conexão(ões) ociosa(s) fechada(s)");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Uma rodada num estado novo, sem catálogo iniciado nem conexão aberta,
    /// não falha nem bloqueia.
    #[tokio::test]
    async fn a_round_on_a_fresh_state_is_harmless() {
        let dir = std::env::temp_dir().join(format!("octapus_maintenance_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let state = Arc::new(AppState::open(&dir).unwrap());
        state.catalog.init(dir.join("catalog"), Arc::new(|_| {}));

        purge_expired(&state);
        tick_once(&state).await;

        // O laço inteiro fica vivo esperando o próximo minuto
        let running = tokio::spawn(run(Arc::clone(&state)));
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(!running.is_finished());
        running.abort();
        let _ = std::fs::remove_dir_all(&dir);
    }
}
