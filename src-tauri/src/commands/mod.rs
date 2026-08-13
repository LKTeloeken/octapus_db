mod browse;
mod servers;
mod connections;
mod queries;
mod structure;

pub use browse::*;
pub use servers::*;
pub use connections::*;
pub use queries::*;
pub use structure::*;

use std::sync::Arc;

use tauri::State;

use crate::adapters::DatabaseAdapter;
use crate::state::AppState;
use crate::storage::repositories::servers as server_store;

/// Resolve the adapter for a connection, reusing the already-open pool when it
/// exists. The server password is read from the OS keychain *only* on a real
/// cache miss (when the pool has to be created), so the steady-state command
/// path — browsing, listing structure, running queries against an open
/// connection — never prompts the keychain.
///
/// Um adapter em cache pode estar com a conexão morta (idle timeout do servidor,
/// rede que caiu enquanto o app ficou aberto). Antes de devolvê-lo fazemos um
/// ping barato; se falhar, descartamos o cache e reconectamos — só nesse caminho
/// de reconexão real a senha é decifrada.
pub async fn connect_adapter(
    state: &State<'_, AppState>,
    server_id: i64,
    database: &str,
) -> Result<Arc<dyn DatabaseAdapter>, String> {
    if let Some(adapter) = state.connections.get_cached(server_id, database) {
        if adapter.test_connection().await.is_ok() {
            return Ok(adapter);
        }
        state.connections.disconnect(server_id, database);
    }

    let server = server_store::get_by_id(&state.storage, server_id).map_err(|e| e.to_string())?;
    state
        .connections
        .get_or_connect(&server, database)
        .await
        .map_err(|e| e.to_string())
}
