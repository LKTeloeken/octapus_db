use parking_lot::Mutex;
use rusqlite::Connection;

use crate::services::{ConnectionService, QueryService, StructureService};
use crate::storage::health::VaultHealth;

/// Application state managed by Tauri
pub struct AppState {
    /// Local SQLite connection for app storage
    pub storage: Mutex<Connection>,

    /// Resultado do canário, medido uma vez no boot.
    pub vault_health: VaultHealth,

    /// Connection management service
    pub connections: ConnectionService,

    /// Query execution service
    pub queries: QueryService,

    /// Database structure service
    pub structure: StructureService,
}

impl AppState {
    pub fn new(storage_conn: Connection, vault_health: VaultHealth) -> Self {
        Self {
            storage: Mutex::new(storage_conn),
            vault_health,
            connections: ConnectionService::new(),
            queries: QueryService::new(),
            structure: StructureService::new(),
        }
    }
}