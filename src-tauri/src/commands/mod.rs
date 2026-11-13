mod assistant;
mod backup;
mod browse;
mod catalog;
mod cli;
mod export;
mod import;
mod logs;
mod servers;
mod connections;
mod queries;
mod session;
mod settings;
mod structure;
// O updater só existe em desktop (ver lib.rs)
#[cfg(desktop)]
mod updater;

pub use assistant::*;
pub use backup::*;
pub use browse::*;
pub use catalog::*;
pub use cli::*;
pub use export::*;
pub use import::*;
pub use logs::*;
pub use servers::*;
pub use connections::*;
pub use queries::*;
pub use session::*;
pub use settings::*;
pub use structure::*;
#[cfg(desktop)]
pub use updater::*;

use std::sync::Arc;

use tauri::State;

use crate::adapters::DatabaseAdapter;
use crate::error::Result;
use crate::state::AppState;

/// O adapter da conexão, do cache quando já está aberta — ver
/// [`AppState::adapter`].
pub async fn connect_adapter(
    state: &State<'_, AppState>,
    server_id: i64,
    database: &str,
) -> Result<Arc<dyn DatabaseAdapter>> {
    state.adapter(server_id, database).await
}
