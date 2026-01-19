# Refactored Rust Architecture

Here's a clean, scalable structure that separates concerns properly and prepares for multi-database support.

## Directory Structure

```
src/
├── main.rs
├── lib.rs
├── error.rs
├── state.rs
│
├── storage/                    # Local SQLite (saved servers, settings, history)
│   ├── mod.rs
│   ├── database.rs
│   └── repositories/
│       ├── mod.rs
│       └── servers.rs
│
├── adapters/                   # Database adapters (DBs we connect TO)
│   ├── mod.rs
│   ├── traits.rs               # DatabaseAdapter trait
│   └── postgres/
│       ├── mod.rs
│       ├── pool.rs
│       ├── executor.rs
│       ├── metadata.rs
│       └── types.rs
│
├── services/                   # Business logic layer
│   ├── mod.rs
│   ├── connection.rs
│   ├── query.rs
│   └── structure.rs
│
├── commands/                   # Tauri commands (thin wrappers)
│   ├── mod.rs
│   ├── servers.rs
│   ├── connections.rs
│   ├── queries.rs
│   └── structure.rs
│
└── models/
    ├── mod.rs
    ├── server.rs
    ├── query.rs
    └── structure.rs
```

---

## Crucial Files

### `src/error.rs`

```rust
use std::fmt;

#[derive(Debug)]
pub enum Error {
    // Storage errors (local SQLite)
    Storage(String),

    // Connection errors
    Connection(String),
    PoolExhausted,
    ConnectionTimeout,

    // Query errors
    Query(String),
    InvalidQuery(String),

    // Resource errors
    NotFound(String),
    AlreadyExists(String),

    // State errors
    InvalidState(String),

    // Adapter errors
    UnsupportedDatabase(String),
    UnsupportedType(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(msg) => write!(f, "Storage error: {msg}"),
            Self::Connection(msg) => write!(f, "Connection error: {msg}"),
            Self::PoolExhausted => write!(f, "Connection pool exhausted"),
            Self::ConnectionTimeout => write!(f, "Connection timed out"),
            Self::Query(msg) => write!(f, "Query error: {msg}"),
            Self::InvalidQuery(msg) => write!(f, "Invalid query: {msg}"),
            Self::NotFound(msg) => write!(f, "Not found: {msg}"),
            Self::AlreadyExists(msg) => write!(f, "Already exists: {msg}"),
            Self::InvalidState(msg) => write!(f, "Invalid state: {msg}"),
            Self::UnsupportedDatabase(msg) => write!(f, "Unsupported database: {msg}"),
            Self::UnsupportedType(msg) => write!(f, "Unsupported type: {msg}"),
        }
    }
}

impl std::error::Error for Error {}

// Conversion traits
impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Storage(e.to_string())
    }
}

impl From<tokio_postgres::Error> for Error {
    fn from(e: tokio_postgres::Error) -> Self {
        Error::Query(e.to_string())
    }
}

impl From<deadpool_postgres::PoolError> for Error {
    fn from(e: deadpool_postgres::PoolError) -> Self {
        match e {
            deadpool_postgres::PoolError::Timeout(_) => Error::ConnectionTimeout,
            _ => Error::Connection(e.to_string()),
        }
    }
}

// For Tauri serialization
impl serde::Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("Error", 2)?;
        state.serialize_field("code", &self.code())?;
        state.serialize_field("message", &self.to_string())?;
        state.end()
    }
}

impl Error {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Storage(_) => "STORAGE_ERROR",
            Self::Connection(_) => "CONNECTION_ERROR",
            Self::PoolExhausted => "POOL_EXHAUSTED",
            Self::ConnectionTimeout => "CONNECTION_TIMEOUT",
            Self::Query(_) => "QUERY_ERROR",
            Self::InvalidQuery(_) => "INVALID_QUERY",
            Self::NotFound(_) => "NOT_FOUND",
            Self::AlreadyExists(_) => "ALREADY_EXISTS",
            Self::InvalidState(_) => "INVALID_STATE",
            Self::UnsupportedDatabase(_) => "UNSUPPORTED_DATABASE",
            Self::UnsupportedType(_) => "UNSUPPORTED_TYPE",
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
```

---

### `src/state.rs`

```rust
use parking_lot::Mutex;
use rusqlite::Connection;

use crate::services::{ConnectionService, QueryService, StructureService};

/// Application state managed by Tauri
pub struct AppState {
    /// Local SQLite connection for app storage
    pub storage: Mutex<Connection>,

    /// Services (stateless, use connection pools internally)
    pub connections: ConnectionService,
    pub queries: QueryService,
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

### `src/models/mod.rs`

```rust
mod server;
mod query;
mod structure;

pub use server::*;
pub use query::*;
pub use structure::*;
```

---

### `src/models/server.rs`

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseType {
    Postgres,
    Mysql,
    Sqlite,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Server {
    pub id: Option<i64>,
    pub name: String,
    pub db_type: DatabaseType,
    pub host: String,
    pub port: u16,
    pub username: String,
    #[serde(skip_serializing)] // Never send password to frontend
    pub password: String,
    pub default_database: Option<String>,
    pub ssl_enabled: bool,
    pub created_at: i64,
}

/// For creating/updating servers (receives password)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInput {
    pub name: String,
    pub db_type: DatabaseType,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub default_database: Option<String>,
    pub ssl_enabled: Option<bool>,
}

/// Connection identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConnectionId {
    pub server_id: i64,
    pub database: String,
}

impl ConnectionId {
    pub fn new(server_id: i64, database: impl Into<String>) -> Self {
        Self {
            server_id,
            database: database.into(),
        }
    }
}
```

---

### `src/models/query.rs`

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryResult {
    pub columns: Vec<ColumnInfo>,
    pub rows: Vec<Vec<Option<String>>>,
    pub row_count: usize,
    pub total_count: Option<i64>,
    pub has_more: bool,
    pub execution_time_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnInfo {
    pub name: String,
    pub type_name: String,
    pub type_oid: Option<u32>, // Postgres-specific, useful for editing
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct QueryOptions {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
    #[serde(default)]
    pub count_total: bool,
    #[serde(default)]
    pub unlimited: bool,
}

fn default_limit() -> i64 {
    500
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatementResult {
    pub affected_rows: u64,
    pub execution_time_ms: u64,
}
```

---

### `src/models/structure.rs`

```rust
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseInfo {
    pub name: String,
    pub size_bytes: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaInfo {
    pub name: String,
    pub table_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableInfo {
    pub name: String,
    pub schema: String,
    pub table_type: TableType,
    pub row_estimate: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TableType {
    Table,
    View,
    MaterializedView,
    Foreign,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnInfo {
    pub name: String,
    pub ordinal: i32,
    pub data_type: String,
    pub is_nullable: bool,
    pub default_value: Option<String>,
    pub is_primary_key: bool,
    pub is_foreign_key: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexInfo {
    pub name: String,
    pub columns: Vec<String>,
    pub is_unique: bool,
    pub is_primary: bool,
    pub index_type: String,
}

/// Full structure for caching (used sparingly)
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseStructure {
    pub schemas: Vec<SchemaStructure>,
    pub fetched_at: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaStructure {
    pub name: String,
    pub tables: Vec<TableStructure>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableStructure {
    pub name: String,
    pub table_type: TableType,
    pub columns: Vec<ColumnInfo>,
}
```

---

### `src/adapters/traits.rs`

```rust
use async_trait::async_trait;

use crate::error::Result;
use crate::models::{
    ColumnInfo, DatabaseInfo, IndexInfo, QueryOptions, QueryResult,
    SchemaInfo, StatementResult, TableInfo,
};

/// Core trait that all database adapters must implement
#[async_trait]
pub trait DatabaseAdapter: Send + Sync {
    // ─────────────────────────────────────────────────────────────────────
    // Query Execution
    // ─────────────────────────────────────────────────────────────────────

    async fn execute_query(
        &self,
        query: &str,
        options: QueryOptions,
    ) -> Result<QueryResult>;

    async fn execute_statement(&self, statement: &str) -> Result<StatementResult>;

    async fn execute_transaction(
        &self,
        statements: Vec<String>,
    ) -> Result<Vec<StatementResult>>;

    // ─────────────────────────────────────────────────────────────────────
    // Metadata - Lazy Loading
    // ─────────────────────────────────────────────────────────────────────

    async fn list_databases(&self) -> Result<Vec<DatabaseInfo>>;

    async fn list_schemas(&self) -> Result<Vec<SchemaInfo>>;

    async fn list_tables(&self, schema: &str) -> Result<Vec<TableInfo>>;

    async fn list_columns(&self, schema: &str, table: &str) -> Result<Vec<ColumnInfo>>;

    async fn list_indexes(&self, schema: &str, table: &str) -> Result<Vec<IndexInfo>>;

    // ─────────────────────────────────────────────────────────────────────
    // Connection
    // ─────────────────────────────────────────────────────────────────────

    async fn test_connection(&self) -> Result<()>;

    async fn cancel_query(&self, query_id: &str) -> Result<()>;
}

/// Pool statistics for monitoring
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PoolStats {
    pub size: usize,
    pub available: usize,
    pub in_use: usize,
    pub waiting: usize,
}

/// Trait for adapters that use connection pooling
#[async_trait]
pub trait PooledAdapter: DatabaseAdapter {
    fn pool_stats(&self) -> PoolStats;
}
```

---

### `src/adapters/mod.rs`

```rust
mod traits;
pub mod postgres;

pub use traits::*;

use std::sync::Arc;

use crate::error::{Error, Result};
use crate::models::{DatabaseType, Server};

/// Create an adapter for the given server and database
pub fn create_adapter(
    server: &Server,
    database: &str,
) -> Result<Arc<dyn DatabaseAdapter>> {
    match server.db_type {
        DatabaseType::Postgres => {
            let adapter = postgres::PostgresAdapter::new(server, database)?;
            Ok(Arc::new(adapter))
        }
        DatabaseType::Mysql => {
            Err(Error::UnsupportedDatabase("MySQL support coming soon".into()))
        }
        DatabaseType::Sqlite => {
            Err(Error::UnsupportedDatabase("SQLite support coming soon".into()))
        }
    }
}
```

---

### `src/adapters/postgres/mod.rs`

```rust
mod pool;
mod executor;
mod metadata;
mod types;

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use deadpool_postgres::Pool;

use crate::error::Result;
use crate::models::*;
use crate::adapters::{DatabaseAdapter, PoolStats, PooledAdapter};

pub use pool::create_pool;

pub struct PostgresAdapter {
    pool: Pool,
    database: String,
}

impl PostgresAdapter {
    pub fn new(server: &Server, database: &str) -> Result<Self> {
        let pool = pool::create_pool(server, database)?;
        Ok(Self {
            pool,
            database: database.to_string(),
        })
    }
}

#[async_trait]
impl DatabaseAdapter for PostgresAdapter {
    async fn execute_query(
        &self,
        query: &str,
        options: QueryOptions,
    ) -> Result<QueryResult> {
        executor::execute_query(&self.pool, query, options).await
    }

    async fn execute_statement(&self, statement: &str) -> Result<StatementResult> {
        executor::execute_statement(&self.pool, statement).await
    }

    async fn execute_transaction(
        &self,
        statements: Vec<String>,
    ) -> Result<Vec<StatementResult>> {
        executor::execute_transaction(&self.pool, statements).await
    }

    async fn list_databases(&self) -> Result<Vec<DatabaseInfo>> {
        metadata::list_databases(&self.pool).await
    }

    async fn list_schemas(&self) -> Result<Vec<SchemaInfo>> {
        metadata::list_schemas(&self.pool).await
    }

    async fn list_tables(&self, schema: &str) -> Result<Vec<TableInfo>> {
        metadata::list_tables(&self.pool, schema).await
    }

    async fn list_columns(&self, schema: &str, table: &str) -> Result<Vec<ColumnInfo>> {
        metadata::list_columns(&self.pool, schema, table).await
    }

    async fn list_indexes(&self, schema: &str, table: &str) -> Result<Vec<IndexInfo>> {
        metadata::list_indexes(&self.pool, schema, table).await
    }

    async fn test_connection(&self) -> Result<()> {
        let client = self.pool.get().await?;
        client.query_one("SELECT 1", &[]).await?;
        Ok(())
    }

    async fn cancel_query(&self, query_id: &str) -> Result<()> {
        // Implementation: parse query_id as PID, call pg_cancel_backend
        todo!("Implement query cancellation")
    }
}

impl PooledAdapter for PostgresAdapter {
    fn pool_stats(&self) -> PoolStats {
        let status = self.pool.status();
        PoolStats {
            size: status.size,
            available: status.available as usize,
            in_use: status.size - status.available as usize,
            waiting: status.waiting,
        }
    }
}
```

---

### `src/adapters/postgres/pool.rs`

```rust
use std::time::Duration;

use deadpool_postgres::{Config, ManagerConfig, Pool, RecyclingMethod, Runtime};
use tokio_postgres::NoTls;

use crate::error::{Error, Result};
use crate::models::Server;

const POOL_MAX_SIZE: usize = 16;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const IDLE_TIMEOUT: Duration = Duration::from_secs(300);

pub fn create_pool(server: &Server, database: &str) -> Result<Pool> {
    let mut cfg = Config::new();

    cfg.host = Some(server.host.clone());
    cfg.port = Some(server.port);
    cfg.user = Some(server.username.clone());
    cfg.password = Some(server.password.clone());
    cfg.dbname = Some(database.to_string());
    cfg.connect_timeout = Some(CONNECT_TIMEOUT);

    cfg.manager = Some(ManagerConfig {
        recycling_method: RecyclingMethod::Fast,
    });

    cfg.pool = Some(deadpool_postgres::PoolConfig {
        max_size: POOL_MAX_SIZE,
        timeouts: deadpool::managed::Timeouts {
            wait: Some(Duration::from_secs(30)),
            create: Some(CONNECT_TIMEOUT),
            recycle: Some(Duration::from_secs(5)),
        },
        ..Default::default()
    });

    // TODO: Add SSL support based on server.ssl_enabled
    cfg.create_pool(Some(Runtime::Tokio1), NoTls)
        .map_err(|e| Error::Connection(e.to_string()))
}
```

---

### `src/adapters/postgres/types.rs`

```rust
use tokio_postgres::types::Type;
use tokio_postgres::Row;
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};

/// Extract a PostgreSQL value as a String for JSON transport
pub fn extract_value(row: &Row, idx: usize, pg_type: &Type) -> Option<String> {
    // Macro to reduce boilerplate
    macro_rules! try_get {
        ($t:ty) => {
            row.get::<_, Option<$t>>(idx).map(|v| v.to_string())
        };
    }

    match *pg_type {
        // Integers
        Type::INT2 => try_get!(i16),
        Type::INT4 => try_get!(i32),
        Type::INT8 => try_get!(i64),
        Type::OID => try_get!(u32),

        // Floats
        Type::FLOAT4 => try_get!(f32),
        Type::FLOAT8 => try_get!(f64),

        // Boolean
        Type::BOOL => try_get!(bool),

        // Date/Time
        Type::TIMESTAMP => row.get::<_, Option<NaiveDateTime>>(idx).map(|v| v.to_string()),
        Type::TIMESTAMPTZ => row.get::<_, Option<DateTime<Utc>>>(idx).map(|v| v.to_rfc3339()),
        Type::DATE => row.get::<_, Option<NaiveDate>>(idx).map(|v| v.to_string()),
        Type::TIME | Type::TIMETZ => row.get::<_, Option<NaiveTime>>(idx).map(|v| v.to_string()),

        // Binary
        Type::BYTEA => row.get::<_, Option<Vec<u8>>>(idx).map(hex::encode),

        // Arrays
        Type::INT4_ARRAY => row.get::<_, Option<Vec<i32>>>(idx).map(|v| format!("{v:?}")),
        Type::INT8_ARRAY => row.get::<_, Option<Vec<i64>>>(idx).map(|v| format!("{v:?}")),
        Type::TEXT_ARRAY | Type::VARCHAR_ARRAY => {
            row.get::<_, Option<Vec<String>>>(idx).map(|v| format!("{v:?}"))
        }

        // Text types (most common - try this for unknown types too)
        _ => row.try_get::<_, Option<String>>(idx).ok().flatten(),
    }
}

/// Get type info for column metadata
pub fn type_category(pg_type: &Type) -> &'static str {
    match *pg_type {
        Type::INT2 | Type::INT4 | Type::INT8 | Type::OID => "integer",
        Type::FLOAT4 | Type::FLOAT8 | Type::NUMERIC => "number",
        Type::BOOL => "boolean",
        Type::TIMESTAMP | Type::TIMESTAMPTZ => "timestamp",
        Type::DATE => "date",
        Type::TIME | Type::TIMETZ => "time",
        Type::BYTEA => "binary",
        Type::JSON | Type::JSONB => "json",
        Type::UUID => "uuid",
        _ => "text",
    }
}
```

---

### `src/adapters/postgres/executor.rs`

```rust
use std::time::Instant;

use deadpool_postgres::Pool;
use tokio_postgres::Row;

use crate::error::{Error, Result};
use crate::models::{
    ColumnInfo as QueryColumnInfo, QueryOptions, QueryResult, StatementResult,
};

use super::types::extract_value;

pub async fn execute_query(
    pool: &Pool,
    query: &str,
    options: QueryOptions,
) -> Result<QueryResult> {
    let client = pool.get().await?;
    let trimmed = query.trim().trim_end_matches(';');

    if trimmed.is_empty() {
        return Err(Error::InvalidQuery("Empty query".into()));
    }

    let is_select = is_select_query(trimmed);

    let start = Instant::now();

    // Build paginated query if applicable
    let (exec_query, limit) = if is_select && !options.unlimited {
        let wrapped = format!(
            "SELECT * FROM ({}) AS __q LIMIT {} OFFSET {}",
            trimmed,
            options.limit + 1, // +1 to detect has_more
            options.offset
        );
        (wrapped, Some(options.limit))
    } else {
        (trimmed.to_string(), None)
    };

    let rows = client.query(&exec_query, &[]).await?;
    let execution_time_ms = start.elapsed().as_millis() as u64;

    // Determine if more rows exist
    let (rows_to_process, has_more) = match limit {
        Some(l) if rows.len() as i64 > l => (&rows[..l as usize], true),
        _ => (&rows[..], false),
    };

    // Get total count if requested
    let total_count = if options.count_total && is_select {
        let count_query = format!("SELECT COUNT(*) FROM ({trimmed}) AS __c");
        client
            .query_one(&count_query, &[])
            .await
            .ok()
            .and_then(|r| r.get::<_, Option<i64>>(0))
    } else {
        None
    };

    let (columns, result_rows) = process_rows(rows_to_process);

    Ok(QueryResult {
        columns,
        rows: result_rows,
        row_count: rows_to_process.len(),
        total_count,
        has_more,
        execution_time_ms,
    })
}

pub async fn execute_statement(pool: &Pool, statement: &str) -> Result<StatementResult> {
    let client = pool.get().await?;
    let start = Instant::now();

    let affected = client.execute(statement, &[]).await?;

    Ok(StatementResult {
        affected_rows: affected,
        execution_time_ms: start.elapsed().as_millis() as u64,
    })
}

pub async fn execute_transaction(
    pool: &Pool,
    statements: Vec<String>,
) -> Result<Vec<StatementResult>> {
    let mut client = pool.get().await?;
    let tx = client.transaction().await?;

    let mut results = Vec::with_capacity(statements.len());

    for stmt in &statements {
        let start = Instant::now();
        let affected = tx.execute(stmt.as_str(), &[]).await?;
        results.push(StatementResult {
            affected_rows: affected,
            execution_time_ms: start.elapsed().as_millis() as u64,
        });
    }

    tx.commit().await?;
    Ok(results)
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn is_select_query(query: &str) -> bool {
    let first_word = query.split_whitespace().next().unwrap_or("");
    matches!(first_word.to_uppercase().as_str(), "SELECT" | "TABLE" | "WITH")
}

fn process_rows(rows: &[Row]) -> (Vec<QueryColumnInfo>, Vec<Vec<Option<String>>>) {
    if rows.is_empty() {
        return (vec![], vec![]);
    }

    let first = &rows[0];
    let columns: Vec<QueryColumnInfo> = first
        .columns()
        .iter()
        .map(|c| QueryColumnInfo {
            name: c.name().to_string(),
            type_name: c.type_().name().to_string(),
            type_oid: Some(c.type_().oid()),
        })
        .collect();

    let types: Vec<_> = first.columns().iter().map(|c| c.type_().clone()).collect();

    let result_rows = rows
        .iter()
        .map(|row| {
            types
                .iter()
                .enumerate()
                .map(|(i, t)| extract_value(row, i, t))
                .collect()
        })
        .collect();

    (columns, result_rows)
}
```

---

### `src/adapters/postgres/metadata.rs`

```rust
use deadpool_postgres::Pool;

use crate::error::Result;
use crate::models::{
    ColumnInfo, DatabaseInfo, IndexInfo, SchemaInfo, TableInfo, TableType,
};

pub async fn list_databases(pool: &Pool) -> Result<Vec<DatabaseInfo>> {
    let client = pool.get().await?;

    let rows = client
        .query(
            r#"
            SELECT d.datname, pg_database_size(d.datname) as size_bytes
            FROM pg_database d
            WHERE d.datistemplate = false
            ORDER BY d.datname
            "#,
            &[],
        )
        .await?;

    Ok(rows
        .iter()
        .map(|r| DatabaseInfo {
            name: r.get(0),
            size_bytes: r.get(1),
        })
        .collect())
}

pub async fn list_schemas(pool: &Pool) -> Result<Vec<SchemaInfo>> {
    let client = pool.get().await?;

    let rows = client
        .query(
            r#"
            SELECT
                n.nspname,
                COUNT(c.oid)::bigint as table_count
            FROM pg_namespace n
            LEFT JOIN pg_class c ON c.relnamespace = n.oid AND c.relkind IN ('r', 'v', 'm')
            WHERE n.nspname NOT IN ('pg_toast', 'pg_catalog', 'information_schema')
            GROUP BY n.nspname
            ORDER BY n.nspname
            "#,
            &[],
        )
        .await?;

    Ok(rows
        .iter()
        .map(|r| SchemaInfo {
            name: r.get(0),
            table_count: r.get(1),
        })
        .collect())
}

pub async fn list_tables(pool: &Pool, schema: &str) -> Result<Vec<TableInfo>> {
    let client = pool.get().await?;

    let rows = client
        .query(
            r#"
            SELECT
                c.relname,
                n.nspname,
                CASE c.relkind
                    WHEN 'r' THEN 'table'
                    WHEN 'v' THEN 'view'
                    WHEN 'm' THEN 'materialized_view'
                    WHEN 'f' THEN 'foreign'
                END as table_type,
                c.reltuples::bigint as row_estimate
            FROM pg_class c
            JOIN pg_namespace n ON n.oid = c.relnamespace
            WHERE n.nspname = $1 AND c.relkind IN ('r', 'v', 'm', 'f')
            ORDER BY c.relname
            "#,
            &[&schema],
        )
        .await?;

    Ok(rows
        .iter()
        .map(|r| {
            let type_str: String = r.get(2);
            TableInfo {
                name: r.get(0),
                schema: r.get(1),
                table_type: match type_str.as_str() {
                    "view" => TableType::View,
                    "materialized_view" => TableType::MaterializedView,
                    "foreign" => TableType::Foreign,
                    _ => TableType::Table,
                },
                row_estimate: r.get(3),
            }
        })
        .collect())
}

pub async fn list_columns(pool: &Pool, schema: &str, table: &str) -> Result<Vec<ColumnInfo>> {
    let client = pool.get().await?;

    let rows = client
        .query(
            r#"
            SELECT
                a.attname,
                a.attnum,
                format_type(a.atttypid, a.atttypmod),
                NOT a.attnotnull,
                pg_get_expr(d.adbin, d.adrelid),
                EXISTS (
                    SELECT 1 FROM pg_constraint c
                    WHERE c.conrelid = a.attrelid
                    AND a.attnum = ANY(c.conkey)
                    AND c.contype = 'p'
                ) as is_pk,
                EXISTS (
                    SELECT 1 FROM pg_constraint c
                    WHERE c.conrelid = a.attrelid
                    AND a.attnum = ANY(c.conkey)
                    AND c.contype = 'f'
                ) as is_fk
            FROM pg_attribute a
            JOIN pg_class c ON c.oid = a.attrelid
            JOIN pg_namespace n ON n.oid = c.relnamespace
            LEFT JOIN pg_attrdef d ON d.adrelid = a.attrelid AND d.adnum = a.attnum
            WHERE n.nspname = $1
              AND c.relname = $2
              AND a.attnum > 0
              AND NOT a.attisdropped
            ORDER BY a.attnum
            "#,
            &[&schema, &table],
        )
        .await?;

    Ok(rows
        .iter()
        .map(|r| ColumnInfo {
            name: r.get(0),
            ordinal: r.get::<_, i16>(1) as i32,
            data_type: r.get(2),
            is_nullable: r.get(3),
            default_value: r.get(4),
            is_primary_key: r.get(5),
            is_foreign_key: r.get(6),
        })
        .collect())
}

pub async fn list_indexes(pool: &Pool, schema: &str, table: &str) -> Result<Vec<IndexInfo>> {
    let client = pool.get().await?;

    let rows = client
        .query(
            r#"
            SELECT
                i.relname,
                array_agg(a.attname ORDER BY array_position(ix.indkey, a.attnum)),
                ix.indisunique,
                ix.indisprimary,
                am.amname
            FROM pg_index ix
            JOIN pg_class i ON i.oid = ix.indexrelid
            JOIN pg_class t ON t.oid = ix.indrelid
            JOIN pg_namespace n ON n.oid = t.relnamespace
            JOIN pg_am am ON am.oid = i.relam
            JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = ANY(ix.indkey)
            WHERE n.nspname = $1 AND t.relname = $2
            GROUP BY i.relname, ix.indisunique, ix.indisprimary, am.amname
            ORDER BY i.relname
            "#,
            &[&schema, &table],
        )
        .await?;

    Ok(rows
        .iter()
        .map(|r| IndexInfo {
            name: r.get(0),
            columns: r.get(1),
            is_unique: r.get(2),
            is_primary: r.get(3),
            index_type: r.get(4),
        })
        .collect())
}
```

---

### `src/services/mod.rs`

```rust
mod connection;
mod query;
mod structure;

pub use connection::ConnectionService;
pub use query::QueryService;
pub use structure::StructureService;
```

---

### `src/services/connection.rs`

```rust
use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;

use crate::adapters::{create_adapter, DatabaseAdapter, PoolStats, PooledAdapter};
use crate::error::{Error, Result};
use crate::models::{ConnectionId, Server};

pub struct ConnectionService {
    adapters: RwLock<HashMap<ConnectionId, Arc<dyn DatabaseAdapter>>>,
}

impl ConnectionService {
    pub fn new() -> Self {
        Self {
            adapters: RwLock::new(HashMap::new()),
        }
    }

    /// Get or create an adapter for the given connection
    pub fn get_or_connect(
        &self,
        server: &Server,
        database: &str,
    ) -> Result<Arc<dyn DatabaseAdapter>> {
        let server_id = server.id.ok_or(Error::InvalidState("Server has no ID".into()))?;
        let conn_id = ConnectionId::new(server_id, database);

        // Fast path: adapter exists
        {
            let adapters = self.adapters.read();
            if let Some(adapter) = adapters.get(&conn_id) {
                return Ok(Arc::clone(adapter));
            }
        }

        // Slow path: create adapter
        let adapter = create_adapter(server, database)?;

        let mut adapters = self.adapters.write();
        adapters.insert(conn_id, Arc::clone(&adapter));

        Ok(adapter)
    }

    /// Disconnect from a specific database
    pub fn disconnect(&self, server_id: i64, database: &str) {
        let conn_id = ConnectionId::new(server_id, database);
        let mut adapters = self.adapters.write();
        adapters.remove(&conn_id);
    }

    /// Disconnect all databases for a server
    pub fn disconnect_server(&self, server_id: i64) {
        let mut adapters = self.adapters.write();
        adapters.retain(|k, _| k.server_id != server_id);
    }

    /// Get pool stats for a connection
    pub fn pool_stats(&self, server_id: i64, database: &str) -> Option<PoolStats> {
        let conn_id = ConnectionId::new(server_id, database);
        let adapters = self.adapters.read();

        adapters.get(&conn_id).and_then(|adapter| {
            // Try to downcast to PooledAdapter
            // This is a bit awkward; in real code you might use a different approach
            None // TODO: implement proper downcasting
        })
    }
}
```

---

### `src/commands/mod.rs`

```rust
mod servers;
mod connections;
mod queries;
mod structure;

pub use servers::*;
pub use connections::*;
pub use queries::*;
pub use structure::*;

/// Helper to get server from storage
pub(crate) fn get_server(
    state: &crate::state::AppState,
    server_id: i64,
) -> Result<crate::models::Server, String> {
    crate::storage::repositories::servers::get_by_id(&state.storage, server_id)
        .map_err(|e| e.to_string())
}
```

---

### `src/commands/queries.rs`

```rust
use tauri::State;

use crate::models::{QueryOptions, QueryResult, StatementResult};
use crate::state::AppState;

use super::get_server;

#[tauri::command]
pub async fn execute_query(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
    query: String,
    options: Option<QueryOptions>,
) -> Result<QueryResult, String> {
    let server = get_server(&state, server_id)?;

    let adapter = state
        .connections
        .get_or_connect(&server, &database)
        .map_err(|e| e.to_string())?;

    adapter
        .execute_query(&query, options.unwrap_or_default())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn execute_statement(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
    statement: String,
) -> Result<StatementResult, String> {
    let server = get_server(&state, server_id)?;

    let adapter = state
        .connections
        .get_or_connect(&server, &database)
        .map_err(|e| e.to_string())?;

    adapter
        .execute_statement(&statement)
        .await
        .map_err(|e| e.to_string())
}
```

---

### `src/commands/structure.rs`

```rust
use tauri::State;

use crate::models::{ColumnInfo, DatabaseInfo, IndexInfo, SchemaInfo, TableInfo};
use crate::state::AppState;

use super::get_server;

#[tauri::command]
pub async fn list_databases(
    state: State<'_, AppState>,
    server_id: i64,
) -> Result<Vec<DatabaseInfo>, String> {
    let server = get_server(&state, server_id)?;
    let db = server.default_database.as_deref().unwrap_or("postgres");

    let adapter = state
        .connections
        .get_or_connect(&server, db)
        .map_err(|e| e.to_string())?;

    adapter.list_databases().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_schemas(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
) -> Result<Vec<SchemaInfo>, String> {
    let server = get_server(&state, server_id)?;

    let adapter = state
        .connections
        .get_or_connect(&server, &database)
        .map_err(|e| e.to_string())?;

    adapter.list_schemas().await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_tables(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
    schema: String,
) -> Result<Vec<TableInfo>, String> {
    let server = get_server(&state, server_id)?;

    let adapter = state
        .connections
        .get_or_connect(&server, &database)
        .map_err(|e| e.to_string())?;

    adapter.list_tables(&schema).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_columns(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
    schema: String,
    table: String,
) -> Result<Vec<ColumnInfo>, String> {
    let server = get_server(&state, server_id)?;

    let adapter = state
        .connections
        .get_or_connect(&server, &database)
        .map_err(|e| e.to_string())?;

    adapter
        .list_columns(&schema, &table)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_indexes(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
    schema: String,
    table: String,
) -> Result<Vec<IndexInfo>, String> {
    let server = get_server(&state, server_id)?;

    let adapter = state
        .connections
        .get_or_connect(&server, &database)
        .map_err(|e| e.to_string())?;

    adapter
        .list_indexes(&schema, &table)
        .await
        .map_err(|e| e.to_string())
}
```

---

### `src/lib.rs`

```rust
mod error;
mod state;
mod models;
mod adapters;
mod services;
mod storage;
mod commands;

use tauri::{Builder, Manager};

use state::AppState;
use storage::database::init_storage;

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

            let storage_conn = init_storage(&app_data_dir.join("app.db"))
                .expect("Failed to initialize storage");

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

## Key Improvements

| Aspect             | Before                  | After                               |
| ------------------ | ----------------------- | ----------------------------------- |
| **Separation**     | Everything in one file  | Clear module boundaries             |
| **Extensibility**  | PostgreSQL-only         | `DatabaseAdapter` trait for any DB  |
| **Error Handling** | String errors           | Typed errors with codes             |
| **State**          | Global HashMap          | Service layer with proper lifetimes |
| **Commands**       | Business logic mixed in | Thin wrappers calling services      |
| **Lazy Loading**   | Full structure at once  | Individual endpoints per level      |

## Files Left to Create

These follow the same patterns shown above:

- `src/storage/mod.rs` — re-exports
- `src/storage/database.rs` — SQLite init (you have this already)
- `src/storage/repositories/mod.rs` — re-exports
- `src/storage/repositories/servers.rs` — your existing CRUD, cleaned up
- `src/commands/servers.rs` — thin wrappers for server CRUD
- `src/commands/connections.rs` — connect/disconnect/test commands
- `src/services/query.rs` and `structure.rs` — mostly passthrough for now

The structure is ready for streaming (Tauri events for large exports), query cancellation, and multi-database support when you need them.
