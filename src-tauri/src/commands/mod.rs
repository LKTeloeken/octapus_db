mod browse;
mod catalog;
mod export;
mod servers;
mod connections;
mod queries;
mod session;
mod structure;

pub use browse::*;
pub use catalog::*;
pub use export::*;
pub use servers::*;
pub use connections::*;
pub use queries::*;
pub use session::*;
pub use structure::*;

use std::sync::Arc;
use std::time::Duration;

use tauri::State;

use crate::adapters::DatabaseAdapter;
use crate::state::AppState;
use crate::storage::repositories::servers as server_store;

/// Um adapter parado há mais que isto leva um ping antes de ser usado.
const PING_AFTER_IDLE: Duration = Duration::from_secs(30);

/// Resolve the adapter for a connection, reusing the already-open pool when it
/// exists. The server password is read from the OS keychain *only* on a real
/// cache miss (when the pool has to be created), so the steady-state command
/// path — browsing, listing structure, running queries against an open
/// connection — never prompts the keychain.
///
/// Um adapter em cache pode estar com a conexão morta (idle timeout do servidor,
/// rede que caiu enquanto o app ficou aberto). Se ele ficou parado mais de
/// [`PING_AFTER_IDLE`], fazemos um ping antes de devolvê-lo; se falhar,
/// descartamos o cache e reconectamos — só nesse caminho de reconexão real a
/// senha é decifrada. No Postgres o pool ainda testa, por conexão, as que
/// ficaram ociosas (`pool.rs`).
pub async fn connect_adapter(
    state: &State<'_, AppState>,
    server_id: i64,
    database: &str,
) -> Result<Arc<dyn DatabaseAdapter>, String> {
    if let Some(adapter) = state
        .connections
        .get_live(server_id, database, PING_AFTER_IDLE)
        .await
    {
        return Ok(adapter);
    }

    let server = server_store::get_by_id(&state.storage, server_id).map_err(|e| e.to_string())?;
    state
        .connections
        .get_or_connect(&server, database)
        .await
        .map_err(|e| e.to_string())
}
