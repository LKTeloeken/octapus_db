use parking_lot::Mutex;
use rusqlite::Connection;

use crate::services::{CatalogService, ConnectionService, QueryService, StructureService};

/// Application state managed by Tauri
pub struct AppState {
    /// Local SQLite connection for app storage
    pub storage: Mutex<Connection>,

    /// Connection management service
    pub connections: ConnectionService,

    /// Query execution service
    pub queries: QueryService,

    /// Database structure service
    pub structure: StructureService,

    /// Catálogo de metadados (árvore, busca, autocomplete) — ver services/catalog.rs
    pub catalog: CatalogService,
}

impl AppState {
    pub fn new(storage_conn: Connection) -> Self {
        Self {
            storage: Mutex::new(storage_conn),
            connections: ConnectionService::new(),
            queries: QueryService::new(),
            structure: StructureService::new(),
            catalog: CatalogService::default(),
        }
    }
}