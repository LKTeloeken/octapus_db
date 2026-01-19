# Remaining Files

Here are all the missing files to complete the structure:

---

### `src/storage/mod.rs`

```rust
pub mod database;
pub mod repositories;

pub use database::init_storage;
```

---

### `src/storage/database.rs`

```rust
use std::path::Path;

use rusqlite::{Connection, Result};

pub fn init_storage<P: AsRef<Path>>(db_path: P) -> Result<Connection> {
    let conn = Connection::open(db_path)?;

    conn.execute_batch(
        r#"
        PRAGMA journal_mode = WAL;
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS servers (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            name            TEXT NOT NULL,
            db_type         TEXT NOT NULL DEFAULT 'postgres',
            host            TEXT NOT NULL,
            port            INTEGER NOT NULL DEFAULT 5432,
            username        TEXT NOT NULL,
            password        TEXT NOT NULL,
            default_database TEXT,
            ssl_enabled     INTEGER NOT NULL DEFAULT 0,
            created_at      INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
        );

        CREATE INDEX IF NOT EXISTS idx_servers_name ON servers(name);
        "#,
    )?;

    Ok(conn)
}
```

---

### `src/storage/repositories/mod.rs`

```rust
pub mod servers;

pub use servers::*;
```

---

### `src/storage/repositories/servers.rs`

```rust
use std::time::{SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;
use rusqlite::{params, Connection};

use crate::error::{Error, Result};
use crate::models::{DatabaseType, Server, ServerInput};

/// Get all servers
pub fn get_all(storage: &Mutex<Connection>) -> Result<Vec<Server>> {
    let conn = storage.lock();

    let mut stmt = conn.prepare(
        r#"
        SELECT id, name, db_type, host, port, username, password,
               default_database, ssl_enabled, created_at
        FROM servers
        ORDER BY name ASC
        "#,
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(Server {
            id: Some(row.get(0)?),
            name: row.get(1)?,
            db_type: parse_db_type(row.get::<_, String>(2)?),
            host: row.get(3)?,
            port: row.get(4)?,
            username: row.get(5)?,
            password: row.get(6)?,
            default_database: row.get(7)?,
            ssl_enabled: row.get::<_, i32>(8)? != 0,
            created_at: row.get(9)?,
        })
    })?;

    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::Storage(e.to_string()))
}

/// Get a server by ID
pub fn get_by_id(storage: &Mutex<Connection>, id: i64) -> Result<Server> {
    let conn = storage.lock();

    let mut stmt = conn.prepare(
        r#"
        SELECT id, name, db_type, host, port, username, password,
               default_database, ssl_enabled, created_at
        FROM servers
        WHERE id = ?
        "#,
    )?;

    stmt.query_row([id], |row| {
        Ok(Server {
            id: Some(row.get(0)?),
            name: row.get(1)?,
            db_type: parse_db_type(row.get::<_, String>(2)?),
            host: row.get(3)?,
            port: row.get(4)?,
            username: row.get(5)?,
            password: row.get(6)?,
            default_database: row.get(7)?,
            ssl_enabled: row.get::<_, i32>(8)? != 0,
            created_at: row.get(9)?,
        })
    })
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            Error::NotFound(format!("Server with id {} not found", id))
        }
        _ => Error::Storage(e.to_string()),
    })
}

/// Create a new server
pub fn create(storage: &Mutex<Connection>, input: ServerInput) -> Result<Server> {
    let conn = storage.lock();

    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| Error::Storage(e.to_string()))?
        .as_secs() as i64;

    let db_type_str = db_type_to_string(&input.db_type);
    let ssl_enabled = input.ssl_enabled.unwrap_or(false) as i32;

    let mut stmt = conn.prepare(
        r#"
        INSERT INTO servers (name, db_type, host, port, username, password,
                            default_database, ssl_enabled, created_at)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        RETURNING id, name, db_type, host, port, username, password,
                  default_database, ssl_enabled, created_at
        "#,
    )?;

    stmt.query_row(
        params![
            input.name,
            db_type_str,
            input.host,
            input.port,
            input.username,
            input.password,
            input.default_database,
            ssl_enabled,
            created_at,
        ],
        |row| {
            Ok(Server {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                db_type: parse_db_type(row.get::<_, String>(2)?),
                host: row.get(3)?,
                port: row.get(4)?,
                username: row.get(5)?,
                password: row.get(6)?,
                default_database: row.get(7)?,
                ssl_enabled: row.get::<_, i32>(8)? != 0,
                created_at: row.get(9)?,
            })
        },
    )
    .map_err(|e| Error::Storage(e.to_string()))
}

/// Update an existing server
pub fn update(storage: &Mutex<Connection>, id: i64, input: ServerInput) -> Result<Server> {
    let conn = storage.lock();

    let db_type_str = db_type_to_string(&input.db_type);
    let ssl_enabled = input.ssl_enabled.unwrap_or(false) as i32;

    let mut stmt = conn.prepare(
        r#"
        UPDATE servers
        SET name = ?1, db_type = ?2, host = ?3, port = ?4, username = ?5,
            password = ?6, default_database = ?7, ssl_enabled = ?8
        WHERE id = ?9
        RETURNING id, name, db_type, host, port, username, password,
                  default_database, ssl_enabled, created_at
        "#,
    )?;

    stmt.query_row(
        params![
            input.name,
            db_type_str,
            input.host,
            input.port,
            input.username,
            input.password,
            input.default_database,
            ssl_enabled,
            id,
        ],
        |row| {
            Ok(Server {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                db_type: parse_db_type(row.get::<_, String>(2)?),
                host: row.get(3)?,
                port: row.get(4)?,
                username: row.get(5)?,
                password: row.get(6)?,
                default_database: row.get(7)?,
                ssl_enabled: row.get::<_, i32>(8)? != 0,
                created_at: row.get(9)?,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            Error::NotFound(format!("Server with id {} not found", id))
        }
        _ => Error::Storage(e.to_string()),
    })
}

/// Delete a server
pub fn delete(storage: &Mutex<Connection>, id: i64) -> Result<()> {
    let conn = storage.lock();

    let affected = conn.execute("DELETE FROM servers WHERE id = ?", [id])?;

    if affected == 0 {
        return Err(Error::NotFound(format!("Server with id {} not found", id)));
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn parse_db_type(s: String) -> DatabaseType {
    match s.to_lowercase().as_str() {
        "mysql" => DatabaseType::Mysql,
        "sqlite" => DatabaseType::Sqlite,
        _ => DatabaseType::Postgres,
    }
}

fn db_type_to_string(db_type: &DatabaseType) -> &'static str {
    match db_type {
        DatabaseType::Postgres => "postgres",
        DatabaseType::Mysql => "mysql",
        DatabaseType::Sqlite => "sqlite",
    }
}
```

---

### `src/commands/servers.rs`

```rust
use tauri::State;

use crate::models::{Server, ServerInput};
use crate::state::AppState;
use crate::storage::repositories::servers;

#[tauri::command]
pub fn get_all_servers(state: State<'_, AppState>) -> Result<Vec<Server>, String> {
    servers::get_all(&state.storage).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_server(state: State<'_, AppState>, id: i64) -> Result<Server, String> {
    servers::get_by_id(&state.storage, id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn create_server(state: State<'_, AppState>, input: ServerInput) -> Result<Server, String> {
    servers::create(&state.storage, input).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_server(
    state: State<'_, AppState>,
    id: i64,
    input: ServerInput,
) -> Result<Server, String> {
    // Disconnect existing connections when server config changes
    state.connections.disconnect_server(id);

    servers::update(&state.storage, id, input).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_server(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    // Disconnect all connections for this server first
    state.connections.disconnect_server(id);

    servers::delete(&state.storage, id).map_err(|e| e.to_string())
}
```

---

### `src/commands/connections.rs`

```rust
use tauri::State;

use crate::adapters::PoolStats;
use crate::state::AppState;
use crate::storage::repositories::servers;

#[tauri::command]
pub async fn connect(
    state: State<'_, AppState>,
    server_id: i64,
    database: Option<String>,
) -> Result<bool, String> {
    let server = servers::get_by_id(&state.storage, server_id).map_err(|e| e.to_string())?;

    let db = database
        .or(server.default_database.clone())
        .unwrap_or_else(|| "postgres".to_string());

    let adapter = state
        .connections
        .get_or_connect(&server, &db)
        .map_err(|e| e.to_string())?;

    // Verify connection works
    adapter
        .test_connection()
        .await
        .map_err(|e| e.to_string())?;

    Ok(true)
}

#[tauri::command]
pub async fn test_connection(
    state: State<'_, AppState>,
    server_id: i64,
) -> Result<bool, String> {
    let server = servers::get_by_id(&state.storage, server_id).map_err(|e| e.to_string())?;

    let db = server
        .default_database
        .clone()
        .unwrap_or_else(|| "postgres".to_string());

    let adapter = state
        .connections
        .get_or_connect(&server, &db)
        .map_err(|e| e.to_string())?;

    adapter
        .test_connection()
        .await
        .map_err(|e| e.to_string())?;

    Ok(true)
}

#[tauri::command]
pub fn disconnect(
    state: State<'_, AppState>,
    server_id: i64,
    database: Option<String>,
) -> Result<(), String> {
    match database {
        Some(db) => state.connections.disconnect(server_id, &db),
        None => state.connections.disconnect_server(server_id),
    }
    Ok(())
}

#[tauri::command]
pub fn get_pool_stats(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
) -> Result<Option<PoolStats>, String> {
    Ok(state.connections.pool_stats(server_id, &database))
}
```

---

### `src/services/query.rs`

```rust
use std::sync::Arc;

use crate::adapters::DatabaseAdapter;
use crate::error::Result;
use crate::models::{QueryOptions, QueryResult, StatementResult};

/// Query service - handles query execution logic
///
/// Currently a thin wrapper, but can be extended for:
/// - Query history tracking
/// - Query analysis/explain
/// - Query cancellation management
/// - Result caching
pub struct QueryService {
    // Future: query history storage, active queries tracking, etc.
}

impl QueryService {
    pub fn new() -> Self {
        Self {}
    }

    /// Execute a SELECT query with pagination
    pub async fn execute_query(
        &self,
        adapter: Arc<dyn DatabaseAdapter>,
        query: &str,
        options: QueryOptions,
    ) -> Result<QueryResult> {
        // Future: log to query history, track active queries, etc.
        adapter.execute_query(query, options).await
    }

    /// Execute a statement (INSERT, UPDATE, DELETE, etc.)
    pub async fn execute_statement(
        &self,
        adapter: Arc<dyn DatabaseAdapter>,
        statement: &str,
    ) -> Result<StatementResult> {
        adapter.execute_statement(statement).await
    }

    /// Execute multiple statements in a transaction
    pub async fn execute_transaction(
        &self,
        adapter: Arc<dyn DatabaseAdapter>,
        statements: Vec<String>,
    ) -> Result<Vec<StatementResult>> {
        adapter.execute_transaction(statements).await
    }
}

impl Default for QueryService {
    fn default() -> Self {
        Self::new()
    }
}
```

---

### `src/services/structure.rs`

```rust
use std::sync::Arc;

use crate::adapters::DatabaseAdapter;
use crate::error::Result;
use crate::models::{ColumnInfo, DatabaseInfo, IndexInfo, SchemaInfo, TableInfo};

/// Structure service - handles database metadata/structure
///
/// Currently a thin wrapper, but can be extended for:
/// - Structure caching
/// - Change detection
/// - Background refresh
pub struct StructureService {
    // Future: structure cache, refresh scheduling, etc.
}

impl StructureService {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn list_databases(
        &self,
        adapter: Arc<dyn DatabaseAdapter>,
    ) -> Result<Vec<DatabaseInfo>> {
        adapter.list_databases().await
    }

    pub async fn list_schemas(
        &self,
        adapter: Arc<dyn DatabaseAdapter>,
    ) -> Result<Vec<SchemaInfo>> {
        adapter.list_schemas().await
    }

    pub async fn list_tables(
        &self,
        adapter: Arc<dyn DatabaseAdapter>,
        schema: &str,
    ) -> Result<Vec<TableInfo>> {
        adapter.list_tables(schema).await
    }

    pub async fn list_columns(
        &self,
        adapter: Arc<dyn DatabaseAdapter>,
        schema: &str,
        table: &str,
    ) -> Result<Vec<ColumnInfo>> {
        adapter.list_columns(schema, table).await
    }

    pub async fn list_indexes(
        &self,
        adapter: Arc<dyn DatabaseAdapter>,
        schema: &str,
        table: &str,
    ) -> Result<Vec<IndexInfo>> {
        adapter.list_indexes(schema, table).await
    }
}

impl Default for StructureService {
    fn default() -> Self {
        Self::new()
    }
}
```

---

### Updated `src/commands/mod.rs`

```rust
mod servers;
mod connections;
mod queries;
mod structure;

pub use servers::*;
pub use connections::*;
pub use queries::*;
pub use structure::*;
```

---

### Updated `src/state.rs`

```rust
use parking_lot::Mutex;
use rusqlite::Connection;

use crate::services::{ConnectionService, QueryService, StructureService};

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
}

impl AppState {
    pub fn new(storage_conn: Connection) -> Self {
        Self {
            storage: Mutex::new(storage_conn),
            connections: ConnectionService::new(),
            queries: QueryService::new(),
            structure: StructureService::new(),
        }
    }
}
```

---

### Updated `src/lib.rs`

```rust
mod adapters;
mod commands;
mod error;
mod models;
mod services;
mod state;
mod storage;

use state::AppState;
use storage::init_storage;
use tauri::{Builder, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    Builder::default()
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data directory");

            std::fs::create_dir_all(&app_data_dir)
                .expect("Failed to create app data directory");

            let storage_conn =
                init_storage(&app_data_dir.join("app.db")).expect("Failed to initialize storage");

            app.manage(AppState::new(storage_conn));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Servers
            commands::create_server,
            commands::get_all_servers,
            commands::get_server,
            commands::update_server,
            commands::delete_server,
            // Connections
            commands::connect,
            commands::disconnect,
            commands::test_connection,
            commands::get_pool_stats,
            // Queries
            commands::execute_query,
            commands::execute_statement,
            // Structure (lazy loading)
            commands::list_databases,
            commands::list_schemas,
            commands::list_tables,
            commands::list_columns,
            commands::list_indexes,
        ])
        .plugin(tauri_plugin_opener::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

---

## Cargo.toml Dependencies

Make sure your `Cargo.toml` includes these:

```toml
[dependencies]
tauri = { version = "2", features = [] }
tauri-plugin-opener = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Async runtime
tokio = { version = "1", features = ["full"] }
async-trait = "0.1"

# Database
rusqlite = { version = "0.31", features = ["bundled"] }
tokio-postgres = { version = "0.7", features = ["with-chrono-0_4"] }
deadpool-postgres = "0.14"

# Utils
parking_lot = "0.12"
chrono = { version = "0.4", features = ["serde"] }
hex = "0.4"
```

---

## Quick Verification

After creating all files, run:

```bash
cargo check
```

You may need to fix a few minor import paths depending on your exact setup. The structure is complete and should compile with the dependencies listed above.
