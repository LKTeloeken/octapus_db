# Refactored Database Architecture

I'll implement a clean, extensible architecture with cursor-based pagination. This is a significant refactor, so I'll organize it into logical files.

## Directory Structure

```
src/
├── lib.rs
├── main.rs
├── app_state.rs
├── db.rs
├── models/
│   ├── mod.rs
│   └── server.rs
├── database/
│   ├── mod.rs
│   ├── traits.rs
│   ├── types.rs
│   ├── error.rs
│   ├── connection_manager.rs
│   ├── cursor_manager.rs
│   └── postgres/
│       ├── mod.rs
│       ├── driver.rs
│       ├── value_extractor.rs
│       └── metadata.rs
└── commands/
    ├── mod.rs
    ├── server_commands.rs
    └── query_commands.rs
```

---

## Core Types and Traits

### `src/database/error.rs`

```rust
use std::fmt;

#[derive(Debug)]
pub enum DatabaseError {
    Connection(String),
    Query(String),
    Cursor(String),
    Pool(String),
    Timeout(String),
    NotFound(String),
    InvalidState(String),
    Unsupported(String),
}

impl fmt::Display for DatabaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connection(msg) => write!(f, "Connection error: {}", msg),
            Self::Query(msg) => write!(f, "Query error: {}", msg),
            Self::Cursor(msg) => write!(f, "Cursor error: {}", msg),
            Self::Pool(msg) => write!(f, "Pool error: {}", msg),
            Self::Timeout(msg) => write!(f, "Timeout: {}", msg),
            Self::NotFound(msg) => write!(f, "Not found: {}", msg),
            Self::InvalidState(msg) => write!(f, "Invalid state: {}", msg),
            Self::Unsupported(msg) => write!(f, "Unsupported: {}", msg),
        }
    }
}

impl std::error::Error for DatabaseError {}

impl From<tokio_postgres::Error> for DatabaseError {
    fn from(e: tokio_postgres::Error) -> Self {
        DatabaseError::Query(e.to_string())
    }
}

impl From<deadpool_postgres::PoolError> for DatabaseError {
    fn from(e: deadpool_postgres::PoolError) -> Self {
        DatabaseError::Pool(e.to_string())
    }
}

// For Tauri command responses
impl serde::Serialize for DatabaseError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, DatabaseError>;
```

---

### `src/database/types.rs`

```rust
use serde::{Deserialize, Serialize};

/// Column metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnInfo {
    pub name: String,
    pub type_name: String,
}

/// Query result with pagination metadata
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryResult {
    pub columns: Vec<ColumnInfo>,
    pub rows: Vec<Vec<Option<String>>>,
    pub row_count: usize,
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
    /// Total rows available (if counted)
    pub total_count: Option<i64>,
    /// Whether more rows exist
    pub has_more: bool,
}

/// Result from cursor-based execution
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorQueryResult {
    pub cursor_id: String,
    pub columns: Vec<ColumnInfo>,
    pub rows: Vec<Vec<Option<String>>>,
    pub row_count: usize,
    pub execution_time_ms: u64,
    /// Estimated total (from pg_class.reltuples or EXPLAIN)
    pub estimated_total: Option<i64>,
    pub has_more: bool,
}

/// Result when fetching more rows from cursor
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorFetchResult {
    pub rows: Vec<Vec<Option<String>>>,
    pub row_count: usize,
    pub has_more: bool,
}

/// Query execution options
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryOptions {
    /// Row limit (default: 500)
    pub limit: Option<i64>,
    /// Offset for pagination
    pub offset: Option<i64>,
    /// Count total rows (slower)
    pub count_total: Option<bool>,
}

/// Cursor execution options
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorOptions {
    /// Rows per fetch (default: 1000)
    pub fetch_size: Option<i64>,
    /// Whether to estimate total rows
    pub estimate_total: Option<bool>,
}

/// Connection target
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct ConnectionTarget {
    pub server_id: i32,
    pub database: String,
}

impl ConnectionTarget {
    pub fn new(server_id: i32, database: impl Into<String>) -> Self {
        Self {
            server_id,
            database: database.into(),
        }
    }
}

/// Pool statistics
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PoolStats {
    pub size: usize,
    pub available: usize,
    pub waiting: usize,
}

/// Database structure types for metadata
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaInfo {
    pub name: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TableInfo {
    pub name: String,
    pub schema: String,
    pub table_type: String,
    pub estimated_rows: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnDetail {
    pub name: String,
    pub ordinal_position: i32,
    pub data_type: String,
    pub is_nullable: bool,
    pub column_default: Option<String>,
    pub is_primary_key: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexInfo {
    pub name: String,
    pub schema: String,
    pub table: String,
    pub definition: String,
    pub is_unique: bool,
    pub is_primary: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseInfo {
    pub name: String,
}
```

---

### `src/database/traits.rs`

```rust
use async_trait::async_trait;

use super::error::Result;
use super::types::*;

/// Core trait for database operations
/// Implement this for each database type (PostgreSQL, MySQL, etc.)
#[async_trait]
pub trait DatabaseDriver: Send + Sync {
    /// Get database type identifier
    fn driver_name(&self) -> &'static str;

    // ─────────────────────────────────────────────────────────────────────
    // Query Execution
    // ─────────────────────────────────────────────────────────────────────

    /// Execute a SELECT query with automatic pagination
    async fn execute_query(
        &self,
        target: &ConnectionTarget,
        sql: &str,
        options: Option<QueryOptions>,
    ) -> Result<QueryResult>;

    /// Execute a statement (INSERT, UPDATE, DELETE, DDL)
    async fn execute_statement(
        &self,
        target: &ConnectionTarget,
        sql: &str,
    ) -> Result<u64>;

    /// Execute multiple statements in a transaction
    async fn execute_transaction(
        &self,
        target: &ConnectionTarget,
        statements: Vec<String>,
    ) -> Result<Vec<u64>>;

    // ─────────────────────────────────────────────────────────────────────
    // Cursor Operations
    // ─────────────────────────────────────────────────────────────────────

    /// Execute query with cursor for streaming results
    async fn execute_with_cursor(
        &self,
        target: &ConnectionTarget,
        sql: &str,
        options: Option<CursorOptions>,
    ) -> Result<CursorQueryResult>;

    /// Fetch more rows from an existing cursor
    async fn fetch_cursor(
        &self,
        cursor_id: &str,
        count: i64,
    ) -> Result<CursorFetchResult>;

    /// Close a cursor and release resources
    async fn close_cursor(&self, cursor_id: &str) -> Result<()>;

    /// Close all cursors for a connection target
    async fn close_all_cursors(&self, target: &ConnectionTarget) -> Result<usize>;

    // ─────────────────────────────────────────────────────────────────────
    // Metadata Operations
    // ─────────────────────────────────────────────────────────────────────

    /// List all databases
    async fn list_databases(&self, server_id: i32) -> Result<Vec<DatabaseInfo>>;

    /// List schemas in a database
    async fn list_schemas(&self, target: &ConnectionTarget) -> Result<Vec<SchemaInfo>>;

    /// List tables in a schema
    async fn list_tables(
        &self,
        target: &ConnectionTarget,
        schema: &str,
    ) -> Result<Vec<TableInfo>>;

    /// Get columns for a table
    async fn get_columns(
        &self,
        target: &ConnectionTarget,
        schema: &str,
        table: &str,
    ) -> Result<Vec<ColumnDetail>>;

    /// Get indexes for a table
    async fn get_indexes(
        &self,
        target: &ConnectionTarget,
        schema: &str,
        table: &str,
    ) -> Result<Vec<IndexInfo>>;

    // ─────────────────────────────────────────────────────────────────────
    // Connection Management
    // ─────────────────────────────────────────────────────────────────────

    /// Test connection to server
    async fn test_connection(&self, server_id: i32) -> Result<()>;

    /// Get pool statistics
    async fn get_pool_stats(&self, target: &ConnectionTarget) -> Result<PoolStats>;

    /// Disconnect from a specific database
    async fn disconnect(&self, target: &ConnectionTarget) -> Result<()>;

    /// Disconnect all connections for a server
    async fn disconnect_server(&self, server_id: i32) -> Result<()>;
}

/// Server configuration provider
pub trait ServerConfigProvider: Send + Sync {
    fn get_server(&self, id: i32) -> Result<crate::models::Server>;
}
```

---

## PostgreSQL Implementation

### `src/database/postgres/value_extractor.rs`

```rust
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use tokio_postgres::types::{FromSql, Type};
use tokio_postgres::Row;

use crate::database::types::ColumnInfo;

/// Wrapper for enum values from PostgreSQL
#[derive(Debug)]
pub struct PgEnumValue(pub String);

impl<'a> FromSql<'a> for PgEnumValue {
    fn from_sql(
        _ty: &Type,
        raw: &'a [u8],
    ) -> std::result::Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        let s = std::str::from_utf8(raw)?;
        Ok(PgEnumValue(s.to_string()))
    }

    fn accepts(_ty: &Type) -> bool {
        true
    }
}

/// Extract a value from a row at given index, converting to String
#[inline]
pub fn extract_value(row: &Row, idx: usize, pg_type: &Type) -> Option<String> {
    match pg_type.name() {
        // Integers
        "int2" | "smallint" => row.get::<_, Option<i16>>(idx).map(|v| v.to_string()),
        "int4" | "integer" | "serial" => row.get::<_, Option<i32>>(idx).map(|v| v.to_string()),
        "int8" | "bigint" | "bigserial" => row.get::<_, Option<i64>>(idx).map(|v| v.to_string()),
        "oid" => row.get::<_, Option<u32>>(idx).map(|v| v.to_string()),

        // Floats
        "float4" | "real" => row.get::<_, Option<f32>>(idx).map(|v| v.to_string()),
        "float8" | "double precision" => row.get::<_, Option<f64>>(idx).map(|v| v.to_string()),

        // Boolean
        "bool" | "boolean" => row.get::<_, Option<bool>>(idx).map(|v| v.to_string()),

        // Date/Time
        "timestamp" => row
            .get::<_, Option<NaiveDateTime>>(idx)
            .map(|v| v.format("%Y-%m-%d %H:%M:%S%.f").to_string()),
        "timestamptz" => row
            .get::<_, Option<DateTime<Utc>>>(idx)
            .map(|v| v.format("%Y-%m-%d %H:%M:%S%.f %Z").to_string()),
        "date" => row
            .get::<_, Option<NaiveDate>>(idx)
            .map(|v| v.format("%Y-%m-%d").to_string()),
        "time" | "timetz" => row
            .get::<_, Option<NaiveTime>>(idx)
            .map(|v| v.format("%H:%M:%S%.f").to_string()),

        // Binary
        "bytea" => row.get::<_, Option<Vec<u8>>>(idx).map(hex::encode),

        // Arrays
        "_int4" => row
            .get::<_, Option<Vec<i32>>>(idx)
            .map(|v| format!("{{{}}}", v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(","))),
        "_int8" => row
            .get::<_, Option<Vec<i64>>>(idx)
            .map(|v| format!("{{{}}}", v.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(","))),
        "_text" | "_varchar" => row
            .get::<_, Option<Vec<String>>>(idx)
            .map(|v| format!("{{{}}}", v.iter().map(|x| format!("\"{}\"", x)).collect::<Vec<_>>().join(","))),

        // Text types
        "text" | "varchar" | "char" | "bpchar" | "name" | "citext" => {
            row.get::<_, Option<String>>(idx)
        }

        // Special types
        "uuid" => row.get::<_, Option<String>>(idx),
        "json" | "jsonb" => row.get::<_, Option<String>>(idx),
        "xml" => row.get::<_, Option<String>>(idx),

        // Numeric types
        "numeric" | "decimal" | "money" => row.get::<_, Option<String>>(idx),

        // Network types
        "inet" | "cidr" | "macaddr" => row.get::<_, Option<String>>(idx),

        // Fallback for custom types (enums, etc.)
        _ => row
            .try_get::<_, Option<String>>(idx)
            .ok()
            .flatten()
            .or_else(|| row.get::<_, Option<PgEnumValue>>(idx).map(|v| v.0)),
    }
}

/// Process rows into columns and string values
pub fn process_rows(rows: &[Row]) -> (Vec<ColumnInfo>, Vec<Vec<Option<String>>>) {
    if rows.is_empty() {
        return (vec![], vec![]);
    }

    let columns: Vec<ColumnInfo> = rows[0]
        .columns()
        .iter()
        .map(|c| ColumnInfo {
            name: c.name().to_string(),
            type_name: c.type_().name().to_string(),
        })
        .collect();

    let col_types: Vec<&Type> = rows[0].columns().iter().map(|c| c.type_()).collect();
    let col_count = col_types.len();

    let result_rows: Vec<Vec<Option<String>>> = rows
        .iter()
        .map(|row| {
            (0..col_count)
                .map(|i| extract_value(row, i, col_types[i]))
                .collect()
        })
        .collect();

    (columns, result_rows)
}
```

---

### `src/database/postgres/metadata.rs`

```rust
use tokio_postgres::Client;

use crate::database::error::{DatabaseError, Result};
use crate::database::types::*;

/// Escape a string for use in SQL (prevent SQL injection)
pub fn escape_literal(s: &str) -> String {
    s.replace('\'', "''")
}

pub async fn list_databases(client: &Client) -> Result<Vec<DatabaseInfo>> {
    let rows = client
        .query(
            "SELECT datname FROM pg_database 
             WHERE datistemplate = false 
             ORDER BY datname",
            &[],
        )
        .await?;

    Ok(rows
        .iter()
        .map(|row| DatabaseInfo {
            name: row.get("datname"),
        })
        .collect())
}

pub async fn list_schemas(client: &Client) -> Result<Vec<SchemaInfo>> {
    let rows = client
        .query(
            "SELECT nspname AS name 
             FROM pg_namespace 
             WHERE nspname NOT IN ('pg_toast', 'pg_catalog', 'information_schema')
               AND nspname NOT LIKE 'pg_temp_%'
               AND nspname NOT LIKE 'pg_toast_temp_%'
             ORDER BY nspname",
            &[],
        )
        .await?;

    Ok(rows
        .iter()
        .map(|row| SchemaInfo {
            name: row.get("name"),
        })
        .collect())
}

pub async fn list_tables(client: &Client, schema: &str) -> Result<Vec<TableInfo>> {
    let rows = client
        .query(
            r#"
            SELECT 
                c.relname AS name,
                n.nspname AS schema,
                CASE c.relkind 
                    WHEN 'r' THEN 'TABLE'
                    WHEN 'v' THEN 'VIEW'
                    WHEN 'm' THEN 'MATERIALIZED VIEW'
                    WHEN 'f' THEN 'FOREIGN TABLE'
                    WHEN 'p' THEN 'PARTITIONED TABLE'
                    ELSE 'OTHER'
                END AS table_type,
                c.reltuples::bigint AS estimated_rows
            FROM pg_class c
            JOIN pg_namespace n ON n.oid = c.relnamespace
            WHERE n.nspname = $1
              AND c.relkind IN ('r', 'v', 'm', 'f', 'p')
            ORDER BY c.relname
            "#,
            &[&schema],
        )
        .await?;

    Ok(rows
        .iter()
        .map(|row| {
            let estimated: i64 = row.get("estimated_rows");
            TableInfo {
                name: row.get("name"),
                schema: row.get("schema"),
                table_type: row.get("table_type"),
                estimated_rows: if estimated >= 0 {
                    Some(estimated)
                } else {
                    None
                },
            }
        })
        .collect())
}

pub async fn get_columns(
    client: &Client,
    schema: &str,
    table: &str,
) -> Result<Vec<ColumnDetail>> {
    let rows = client
        .query(
            r#"
            SELECT 
                a.attname AS name,
                a.attnum AS ordinal_position,
                pg_catalog.format_type(a.atttypid, a.atttypmod) AS data_type,
                NOT a.attnotnull AS is_nullable,
                pg_get_expr(d.adbin, d.adrelid) AS column_default,
                COALESCE(pk.is_pk, false) AS is_primary_key
            FROM pg_attribute a
            JOIN pg_class c ON c.oid = a.attrelid
            JOIN pg_namespace n ON n.oid = c.relnamespace
            LEFT JOIN pg_attrdef d ON d.adrelid = a.attrelid AND d.adnum = a.attnum
            LEFT JOIN (
                SELECT 
                    con.conrelid,
                    unnest(con.conkey) AS conkey,
                    true AS is_pk
                FROM pg_constraint con
                WHERE con.contype = 'p'
            ) pk ON pk.conrelid = c.oid AND pk.conkey = a.attnum
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
        .map(|row| ColumnDetail {
            name: row.get("name"),
            ordinal_position: row.get::<_, i16>("ordinal_position") as i32,
            data_type: row.get("data_type"),
            is_nullable: row.get("is_nullable"),
            column_default: row.get("column_default"),
            is_primary_key: row.get("is_primary_key"),
        })
        .collect())
}

pub async fn get_indexes(
    client: &Client,
    schema: &str,
    table: &str,
) -> Result<Vec<IndexInfo>> {
    let rows = client
        .query(
            r#"
            SELECT 
                i.relname AS name,
                n.nspname AS schema,
                t.relname AS table,
                pg_get_indexdef(i.oid) AS definition,
                ix.indisunique AS is_unique,
                ix.indisprimary AS is_primary
            FROM pg_index ix
            JOIN pg_class i ON i.oid = ix.indexrelid
            JOIN pg_class t ON t.oid = ix.indrelid
            JOIN pg_namespace n ON n.oid = t.relnamespace
            WHERE n.nspname = $1
              AND t.relname = $2
            ORDER BY i.relname
            "#,
            &[&schema, &table],
        )
        .await?;

    Ok(rows
        .iter()
        .map(|row| IndexInfo {
            name: row.get("name"),
            schema: row.get("schema"),
            table: row.get("table"),
            definition: row.get("definition"),
            is_unique: row.get("is_unique"),
            is_primary: row.get("is_primary"),
        })
        .collect())
}

/// Estimate row count using EXPLAIN (more accurate than reltuples for complex queries)
pub async fn estimate_query_rows(client: &Client, sql: &str) -> Result<Option<i64>> {
    let explain_sql = format!("EXPLAIN (FORMAT JSON) {}", sql);

    match client.query_one(&explain_sql, &[]).await {
        Ok(row) => {
            let json: serde_json::Value = row.get(0);
            // Navigate: [0]["Plan"]["Plan Rows"]
            let estimate = json
                .get(0)
                .and_then(|v| v.get("Plan"))
                .and_then(|v| v.get("Plan Rows"))
                .and_then(|v| v.as_i64());
            Ok(estimate)
        }
        Err(_) => Ok(None),
    }
}
```

---

### `src/database/postgres/driver.rs`

```rust
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use deadpool_postgres::{Config, Manager, ManagerConfig, Object, Pool, RecyclingMethod, Runtime};
use tokio::sync::RwLock;
use tokio_postgres::NoTls;
use uuid::Uuid;

use crate::database::error::{DatabaseError, Result};
use crate::database::traits::{DatabaseDriver, ServerConfigProvider};
use crate::database::types::*;
use crate::models::Server;

use super::metadata;
use super::value_extractor::process_rows;

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────

const DEFAULT_ROW_LIMIT: i64 = 500;
const DEFAULT_CURSOR_FETCH_SIZE: i64 = 1000;
const POOL_MAX_SIZE: usize = 16;
const CONNECT_TIMEOUT_SECS: u64 = 10;
const CURSOR_IDLE_TIMEOUT_SECS: u64 = 300; // 5 minutes
const MAX_CURSORS_PER_TARGET: usize = 5;
const MAX_TOTAL_CURSORS: usize = 50;

// ─────────────────────────────────────────────────────────────────────────────
// Cursor State
// ─────────────────────────────────────────────────────────────────────────────

struct CursorState {
    id: String,
    target: ConnectionTarget,
    cursor_name: String,
    columns: Vec<ColumnInfo>,
    created_at: Instant,
    last_fetch_at: Instant,
    rows_fetched: usize,
    estimated_total: Option<i64>,
    // The connection is held for the lifetime of the cursor
    // because PostgreSQL cursors require an open transaction
    _connection: Object,
}

// ─────────────────────────────────────────────────────────────────────────────
// PostgreSQL Driver
// ─────────────────────────────────────────────────────────────────────────────

pub struct PostgresDriver {
    config_provider: Arc<dyn ServerConfigProvider>,
    pools: RwLock<HashMap<ConnectionTarget, Pool>>,
    cursors: RwLock<HashMap<String, CursorState>>,
}

impl PostgresDriver {
    pub fn new(config_provider: Arc<dyn ServerConfigProvider>) -> Self {
        Self {
            config_provider,
            pools: RwLock::new(HashMap::new()),
            cursors: RwLock::new(HashMap::new()),
        }
    }

    /// Start the background cleanup task for idle cursors
    pub fn start_cursor_cleanup_task(self: &Arc<Self>) {
        let driver = Arc::clone(self);
        
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            
            loop {
                interval.tick().await;
                
                let now = Instant::now();
                let timeout = Duration::from_secs(CURSOR_IDLE_TIMEOUT_SECS);
                
                let mut cursors = driver.cursors.write().await;
                let expired: Vec<String> = cursors
                    .iter()
                    .filter(|(_, state)| now.duration_since(state.last_fetch_at) > timeout)
                    .map(|(id, _)| id.clone())
                    .collect();
                
                for id in expired {
                    if let Some(state) = cursors.remove(&id) {
                        // Close the cursor on the database
                        let close_sql = format!("CLOSE {}", state.cursor_name);
                        let _ = state._connection.execute(&close_sql, &[]).await;
                        // Transaction will be rolled back when connection is dropped
                        tracing::info!(
                            cursor_id = %id,
                            target = ?state.target,
                            "Closed idle cursor after {} seconds",
                            CURSOR_IDLE_TIMEOUT_SECS
                        );
                    }
                }
            }
        });
    }

    // ─────────────────────────────────────────────────────────────────────
    // Pool Management
    // ─────────────────────────────────────────────────────────────────────

    async fn get_or_create_pool(&self, target: &ConnectionTarget) -> Result<Pool> {
        // Fast path: pool exists
        {
            let pools = self.pools.read().await;
            if let Some(pool) = pools.get(target) {
                return Ok(pool.clone());
            }
        }

        // Slow path: create pool
        let mut pools = self.pools.write().await;

        // Double-check after acquiring write lock
        if let Some(pool) = pools.get(target) {
            return Ok(pool.clone());
        }

        let server = self.config_provider.get_server(target.server_id)?;
        let pool = self.create_pool(&server, &target.database)?;
        pools.insert(target.clone(), pool.clone());

        Ok(pool)
    }

    fn create_pool(&self, server: &Server, database: &str) -> Result<Pool> {
        let mut cfg = Config::new();
        cfg.host = Some(server.host.clone());
        cfg.port = Some(server.port as u16);
        cfg.user = Some(server.username.clone());
        cfg.password = Some(server.password.clone());
        cfg.dbname = Some(database.to_string());
        cfg.connect_timeout = Some(Duration::from_secs(CONNECT_TIMEOUT_SECS));
        cfg.manager = Some(ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        });
        cfg.pool = Some(deadpool_postgres::PoolConfig {
            max_size: POOL_MAX_SIZE,
            timeouts: deadpool::managed::Timeouts {
                wait: Some(Duration::from_secs(30)),
                create: Some(Duration::from_secs(CONNECT_TIMEOUT_SECS)),
                recycle: Some(Duration::from_secs(5)),
            },
            ..Default::default()
        });

        cfg.create_pool(Some(Runtime::Tokio1), NoTls)
            .map_err(|e| DatabaseError::Pool(format!("Failed to create pool: {}", e)))
    }

    async fn get_connection(&self, target: &ConnectionTarget) -> Result<Object> {
        let pool = self.get_or_create_pool(target).await?;
        pool.get()
            .await
            .map_err(|e| DatabaseError::Connection(e.to_string()))
    }

    // ─────────────────────────────────────────────────────────────────────
    // Query Helpers
    // ─────────────────────────────────────────────────────────────────────

    fn is_select_query(sql: &str) -> bool {
        let upper = sql.trim().to_uppercase();
        let first_word = upper.split_whitespace().next().unwrap_or("");
        matches!(first_word, "SELECT" | "TABLE" | "WITH" | "VALUES")
    }

    fn build_paginated_query(sql: &str, limit: i64, offset: i64) -> String {
        let trimmed = sql.trim().trim_end_matches(';');
        format!(
            "SELECT * FROM ({}) AS __paginated_query LIMIT {} OFFSET {}",
            trimmed,
            limit + 1, // Fetch one extra to detect "has_more"
            offset
        )
    }

    // ─────────────────────────────────────────────────────────────────────
    // Cursor Helpers
    // ─────────────────────────────────────────────────────────────────────

    async fn check_cursor_limits(&self, target: &ConnectionTarget) -> Result<()> {
        let cursors = self.cursors.read().await;

        // Check total limit
        if cursors.len() >= MAX_TOTAL_CURSORS {
            return Err(DatabaseError::Cursor(format!(
                "Maximum total cursors ({}) reached. Close some cursors first.",
                MAX_TOTAL_CURSORS
            )));
        }

        // Check per-target limit
        let target_count = cursors
            .values()
            .filter(|c| c.target == *target)
            .count();

        if target_count >= MAX_CURSORS_PER_TARGET {
            return Err(DatabaseError::Cursor(format!(
                "Maximum cursors per database ({}) reached. Close some cursors first.",
                MAX_CURSORS_PER_TARGET
            )));
        }

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// DatabaseDriver Implementation
// ─────────────────────────────────────────────────────────────────────────────

#[async_trait]
impl DatabaseDriver for PostgresDriver {
    fn driver_name(&self) -> &'static str {
        "postgresql"
    }

    // ─────────────────────────────────────────────────────────────────────
    // Query Execution
    // ─────────────────────────────────────────────────────────────────────

    async fn execute_query(
        &self,
        target: &ConnectionTarget,
        sql: &str,
        options: Option<QueryOptions>,
    ) -> Result<QueryResult> {
        let opts = options.unwrap_or_default();
        let client = self.get_connection(target).await?;

        let start = Instant::now();

        let is_select = Self::is_select_query(sql);

        // Build query with pagination for SELECT statements
        let (exec_sql, applied_limit) = if is_select {
            let limit = opts.limit.unwrap_or(DEFAULT_ROW_LIMIT);
            let offset = opts.offset.unwrap_or(0);
            (Self::build_paginated_query(sql, limit, offset), Some(limit))
        } else {
            (sql.trim().trim_end_matches(';').to_string(), None)
        };

        // Execute query
        let rows = client
            .query(&exec_sql, &[])
            .await
            .map_err(|e| DatabaseError::Query(e.to_string()))?;

        let execution_time_ms = start.elapsed().as_millis() as u64;

        // Determine if there are more rows
        let (rows_to_process, has_more) = if let Some(limit) = applied_limit {
            let has_more = rows.len() as i64 > limit;
            let take = if has_more { limit as usize } else { rows.len() };
            (&rows[..take], has_more)
        } else {
            (&rows[..], false)
        };

        // Get total count if requested
        let total_count = if opts.count_total.unwrap_or(false) && is_select {
            let count_sql = format!(
                "SELECT COUNT(*) FROM ({}) AS __count_query",
                sql.trim().trim_end_matches(';')
            );
            client
                .query_one(&count_sql, &[])
                .await
                .ok()
                .and_then(|row| row.get::<_, Option<i64>>(0))
        } else {
            None
        };

        let row_count = rows_to_process.len();
        let (columns, result_rows) = process_rows(rows_to_process);

        Ok(QueryResult {
            columns,
            rows: result_rows,
            row_count,
            execution_time_ms,
            total_count,
            has_more,
        })
    }

    async fn execute_statement(
        &self,
        target: &ConnectionTarget,
        sql: &str,
    ) -> Result<u64> {
        let client = self.get_connection(target).await?;
        client
            .execute(sql, &[])
            .await
            .map_err(|e| DatabaseError::Query(e.to_string()))
    }

    async fn execute_transaction(
        &self,
        target: &ConnectionTarget,
        statements: Vec<String>,
    ) -> Result<Vec<u64>> {
        let mut client = self.get_connection(target).await?;

        let tx = client
            .transaction()
            .await
            .map_err(|e| DatabaseError::Query(format!("Failed to start transaction: {}", e)))?;

        let mut results = Vec::with_capacity(statements.len());

        for stmt in &statements {
            let affected = tx
                .execute(stmt.as_str(), &[])
                .await
                .map_err(|e| DatabaseError::Query(e.to_string()))?;
            results.push(affected);
        }

        tx.commit()
            .await
            .map_err(|e| DatabaseError::Query(format!("Failed to commit: {}", e)))?;

        Ok(results)
    }

    // ─────────────────────────────────────────────────────────────────────
    // Cursor Operations
    // ─────────────────────────────────────────────────────────────────────

    async fn execute_with_cursor(
        &self,
        target: &ConnectionTarget,
        sql: &str,
        options: Option<CursorOptions>,
    ) -> Result<CursorQueryResult> {
        // Check limits before proceeding
        self.check_cursor_limits(target).await?;

        let opts = options.unwrap_or_default();
        let fetch_size = opts.fetch_size.unwrap_or(DEFAULT_CURSOR_FETCH_SIZE);

        // Get a dedicated connection (will be held for cursor lifetime)
        let mut client = self.get_connection(target).await?;

        let start = Instant::now();

        // Start transaction (required for cursors)
        client
            .execute("BEGIN", &[])
            .await
            .map_err(|e| DatabaseError::Cursor(format!("Failed to start transaction: {}", e)))?;

        // Generate unique cursor name
        let cursor_id = Uuid::new_v4().to_string();
        let cursor_name = format!("cursor_{}", cursor_id.replace('-', "_"));

        // Estimate total rows if requested
        let estimated_total = if opts.estimate_total.unwrap_or(false) {
            metadata::estimate_query_rows(&client, sql.trim().trim_end_matches(';')).await?
        } else {
            None
        };

        // Declare cursor
        let declare_sql = format!(
            "DECLARE {} CURSOR FOR {}",
            cursor_name,
            sql.trim().trim_end_matches(';')
        );

        client
            .execute(&declare_sql, &[])
            .await
            .map_err(|e| DatabaseError::Cursor(format!("Failed to declare cursor: {}", e)))?;

        // Fetch first batch
        let fetch_sql = format!("FETCH {} FROM {}", fetch_size + 1, cursor_name);
        let rows = client
            .query(&fetch_sql, &[])
            .await
            .map_err(|e| DatabaseError::Cursor(format!("Failed to fetch: {}", e)))?;

        let execution_time_ms = start.elapsed().as_millis() as u64;

        // Check if there are more rows
        let has_more = rows.len() as i64 > fetch_size;
        let take = if has_more {
            fetch_size as usize
        } else {
            rows.len()
        };

        let (columns, result_rows) = process_rows(&rows[..take]);
        let row_count = result_rows.len();

        // Store cursor state
        let now = Instant::now();
        let state = CursorState {
            id: cursor_id.clone(),
            target: target.clone(),
            cursor_name,
            columns: columns.clone(),
            created_at: now,
            last_fetch_at: now,
            rows_fetched: row_count,
            estimated_total,
            _connection: client,
        };

        self.cursors.write().await.insert(cursor_id.clone(), state);

        Ok(CursorQueryResult {
            cursor_id,
            columns,
            rows: result_rows,
            row_count,
            execution_time_ms,
            estimated_total,
            has_more,
        })
    }

    async fn fetch_cursor(
        &self,
        cursor_id: &str,
        count: i64,
    ) -> Result<CursorFetchResult> {
        let mut cursors = self.cursors.write().await;

        let state = cursors
            .get_mut(cursor_id)
            .ok_or_else(|| DatabaseError::NotFound(format!("Cursor not found: {}", cursor_id)))?;

        // Fetch next batch
        let fetch_sql = format!("FETCH {} FROM {}", count + 1, state.cursor_name);
        let rows = state
            ._connection
            .query(&fetch_sql, &[])
            .await
            .map_err(|e| DatabaseError::Cursor(format!("Failed to fetch: {}", e)))?;

        // Check if there are more rows
        let has_more = rows.len() as i64 > count;
        let take = if has_more { count as usize } else { rows.len() };

        let (_, result_rows) = process_rows(&rows[..take]);
        let row_count = result_rows.len();

        // Update state
        state.last_fetch_at = Instant::now();
        state.rows_fetched += row_count;

        Ok(CursorFetchResult {
            rows: result_rows,
            row_count,
            has_more,
        })
    }

    async fn close_cursor(&self, cursor_id: &str) -> Result<()> {
        let mut cursors = self.cursors.write().await;

        if let Some(state) = cursors.remove(cursor_id) {
            // Close cursor and rollback transaction
            let close_sql = format!("CLOSE {}", state.cursor_name);
            let _ = state._connection.execute(&close_sql, &[]).await;
            let _ = state._connection.execute("ROLLBACK", &[]).await;
            Ok(())
        } else {
            Err(DatabaseError::NotFound(format!(
                "Cursor not found: {}",
                cursor_id
            )))
        }
    }

    async fn close_all_cursors(&self, target: &ConnectionTarget) -> Result<usize> {
        let mut cursors = self.cursors.write().await;

        let to_close: Vec<String> = cursors
            .iter()
            .filter(|(_, state)| state.target == *target)
            .map(|(id, _)| id.clone())
            .collect();

        let count = to_close.len();

        for id in to_close {
            if let Some(state) = cursors.remove(&id) {
                let close_sql = format!("CLOSE {}", state.cursor_name);
                let _ = state._connection.execute(&close_sql, &[]).await;
                let _ = state._connection.execute("ROLLBACK", &[]).await;
            }
        }

        Ok(count)
    }

    // ─────────────────────────────────────────────────────────────────────
    // Metadata Operations
    // ─────────────────────────────────────────────────────────────────────

    async fn list_databases(&self, server_id: i32) -> Result<Vec<DatabaseInfo>> {
        let server = self.config_provider.get_server(server_id)?;
        let default_db = server.default_database.unwrap_or_else(|| "postgres".to_string());
        let target = ConnectionTarget::new(server_id, default_db);
        let client = self.get_connection(&target).await?;
        metadata::list_databases(&client).await
    }

    async fn list_schemas(&self, target: &ConnectionTarget) -> Result<Vec<SchemaInfo>> {
        let client = self.get_connection(target).await?;
        metadata::list_schemas(&client).await
    }

    async fn list_tables(
        &self,
        target: &ConnectionTarget,
        schema: &str,
    ) -> Result<Vec<TableInfo>> {
        let client = self.get_connection(target).await?;
        metadata::list_tables(&client, schema).await
    }

    async fn get_columns(
        &self,
        target: &ConnectionTarget,
        schema: &str,
        table: &str,
    ) -> Result<Vec<ColumnDetail>> {
        let client = self.get_connection(target).await?;
        metadata::get_columns(&client, schema, table).await
    }

    async fn get_indexes(
        &self,
        target: &ConnectionTarget,
        schema: &str,
        table: &str,
    ) -> Result<Vec<IndexInfo>> {
        let client = self.get_connection(target).await?;
        metadata::get_indexes(&client, schema, table).await
    }

    // ─────────────────────────────────────────────────────────────────────
    // Connection Management
    // ─────────────────────────────────────────────────────────────────────

    async fn test_connection(&self, server_id: i32) -> Result<()> {
        let server = self.config_provider.get_server(server_id)?;
        let default_db = server.default_database.unwrap_or_else(|| "postgres".to_string());
        let target = ConnectionTarget::new(server_id, default_db);
        let client = self.get_connection(&target).await?;

        client
            .query_one("SELECT 1", &[])
            .await
            .map_err(|e| DatabaseError::Connection(format!("Connection test failed: {}", e)))?;

        Ok(())
    }

    async fn get_pool_stats(&self, target: &ConnectionTarget) -> Result<PoolStats> {
        let pool = self.get_or_create_pool(target).await?;
        let status = pool.status();

        Ok(PoolStats {
            size: status.size,
            available: status.available as usize,
            waiting: status.waiting,
        })
    }

    async fn disconnect(&self, target: &ConnectionTarget) -> Result<()> {
        // Close all cursors for this target first
        self.close_all_cursors(target).await?;

        // Remove pool
        let mut pools = self.pools.write().await;
        pools.remove(target);

        Ok(())
    }

    async fn disconnect_server(&self, server_id: i32) -> Result<()> {
        // Close all cursors for this server
        {
            let mut cursors = self.cursors.write().await;
            let to_close: Vec<String> = cursors
                .iter()
                .filter(|(_, state)| state.target.server_id == server_id)
                .map(|(id, _)| id.clone())
                .collect();

            for id in to_close {
                if let Some(state) = cursors.remove(&id) {
                    let close_sql = format!("CLOSE {}", state.cursor_name);
                    let _ = state._connection.execute(&close_sql, &[]).await;
                    let _ = state._connection.execute("ROLLBACK", &[]).await;
                }
            }
        }

        // Remove all pools for this server
        let mut pools = self.pools.write().await;
        pools.retain(|target, _| target.server_id != server_id);

        Ok(())
    }
}
```

---

### `src/database/postgres/mod.rs`

```rust
mod driver;
mod metadata;
mod value_extractor;

pub use driver::PostgresDriver;
```

---

## Connection Manager (Global Access)

### `src/database/connection_manager.rs`

```rust
use std::sync::Arc;

use once_cell::sync::OnceCell;

use super::error::{DatabaseError, Result};
use super::postgres::PostgresDriver;
use super::traits::{DatabaseDriver, ServerConfigProvider};
use crate::models::Server;

/// Global database driver instance
static DRIVER: OnceCell<Arc<PostgresDriver>> = OnceCell::new();

/// Initialize the database driver with a config provider
pub fn initialize(config_provider: Arc<dyn ServerConfigProvider>) {
    let driver = Arc::new(PostgresDriver::new(config_provider));
    driver.start_cursor_cleanup_task();
    
    DRIVER
        .set(driver)
        .expect("Database driver already initialized");
}

/// Get the global database driver
pub fn driver() -> Result<&'static Arc<PostgresDriver>> {
    DRIVER
        .get()
        .ok_or_else(|| DatabaseError::InvalidState("Database driver not initialized".to_string()))
}

// ─────────────────────────────────────────────────────────────────────────────
// Config Provider Implementation for AppState
// ─────────────────────────────────────────────────────────────────────────────

use crate::app_state::AppState;
use rusqlite::params;
use std::sync::Mutex;

pub struct SqliteConfigProvider {
    conn: Mutex<rusqlite::Connection>,
}

impl SqliteConfigProvider {
    pub fn new(conn: rusqlite::Connection) -> Self {
        Self {
            conn: Mutex::new(conn),
        }
    }
}

impl ServerConfigProvider for SqliteConfigProvider {
    fn get_server(&self, id: i32) -> Result<Server> {
        let conn = self.conn.lock().map_err(|e| {
            DatabaseError::InvalidState(format!("Failed to lock connection: {}", e))
        })?;

        let mut stmt = conn
            .prepare(
                "SELECT id, name, host, port, username, password, default_database, created_at
                 FROM servers WHERE id = ?",
            )
            .map_err(|e| DatabaseError::Query(e.to_string()))?;

        stmt.query_row(params![id], |row| {
            Ok(Server {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                host: row.get(2)?,
                port: row.get(3)?,
                username: row.get(4)?,
                password: row.get(5)?,
                default_database: row.get(6)?,
                created_at: row.get(7)?,
            })
        })
        .map_err(|e| DatabaseError::NotFound(format!("Server not found: {}", e)))
    }
}
```

---

### `src/database/mod.rs`

```rust
pub mod connection_manager;
pub mod cursor_manager;
pub mod error;
pub mod postgres;
pub mod traits;
pub mod types;

pub use connection_manager::{driver, initialize};
pub use error::{DatabaseError, Result};
pub use types::*;
```

---

## Updated Models

### `src/models/server.rs`

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Server {
    pub id: Option<i32>,
    pub name: String,
    pub host: String,
    pub port: i32,
    pub username: String,
    pub password: String,
    pub default_database: Option<String>,
    pub created_at: i64,
}
```

---

### `src/models/mod.rs`

```rust
mod server;

pub use server::Server;
```

---

## Commands (Tauri Interface)

### `src/commands/query_commands.rs`

```rust
use tauri::State;

use crate::app_state::AppState;
use crate::database::{
    driver, ConnectionTarget, CursorFetchResult, CursorOptions, CursorQueryResult,
    DatabaseError, QueryOptions, QueryResult, Result,
};

// ─────────────────────────────────────────────────────────────────────────────
// Query Execution
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn execute_query(
    server_id: i32,
    database: String,
    sql: String,
    options: Option<QueryOptions>,
) -> Result<QueryResult> {
    let target = ConnectionTarget::new(server_id, database);
    driver()?.execute_query(&target, &sql, options).await
}

#[tauri::command]
pub async fn execute_statement(
    server_id: i32,
    database: String,
    sql: String,
) -> Result<u64> {
    let target = ConnectionTarget::new(server_id, database);
    driver()?.execute_statement(&target, &sql).await
}

#[tauri::command]
pub async fn execute_transaction(
    server_id: i32,
    database: String,
    statements: Vec<String>,
) -> Result<Vec<u64>> {
    let target = ConnectionTarget::new(server_id, database);
    driver()?.execute_transaction(&target, statements).await
}

// ─────────────────────────────────────────────────────────────────────────────
// Cursor Operations
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn execute_with_cursor(
    server_id: i32,
    database: String,
    sql: String,
    options: Option<CursorOptions>,
) -> Result<CursorQueryResult> {
    let target = ConnectionTarget::new(server_id, database);
    driver()?.execute_with_cursor(&target, &sql, options).await
}

#[tauri::command]
pub async fn fetch_cursor(
    cursor_id: String,
    count: Option<i64>,
) -> Result<CursorFetchResult> {
    let count = count.unwrap_or(1000);
    driver()?.fetch_cursor(&cursor_id, count).await
}

#[tauri::command]
pub async fn close_cursor(cursor_id: String) -> Result<()> {
    driver()?.close_cursor(&cursor_id).await
}

#[tauri::command]
pub async fn close_all_cursors(
    server_id: i32,
    database: String,
) -> Result<usize> {
    let target = ConnectionTarget::new(server_id, database);
    driver()?.close_all_cursors(&target).await
}

// ─────────────────────────────────────────────────────────────────────────────
// Metadata
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn list_databases(server_id: i32) -> Result<Vec<crate::database::DatabaseInfo>> {
    driver()?.list_databases(server_id).await
}

#[tauri::command]
pub async fn list_schemas(
    server_id: i32,
    database: String,
) -> Result<Vec<crate::database::SchemaInfo>> {
    let target = ConnectionTarget::new(server_id, database);
    driver()?.list_schemas(&target).await
}

#[tauri::command]
pub async fn list_tables(
    server_id: i32,
    database: String,
    schema: String,
) -> Result<Vec<crate::database::TableInfo>> {
    let target = ConnectionTarget::new(server_id, database);
    driver()?.list_tables(&target, &schema).await
}

#[tauri::command]
pub async fn get_columns(
    server_id: i32,
    database: String,
    schema: String,
    table: String,
) -> Result<Vec<crate::database::ColumnDetail>> {
    let target = ConnectionTarget::new(server_id, database);
    driver()?.get_columns(&target, &schema, &table).await
}

#[tauri::command]
pub async fn get_indexes(
    server_id: i32,
    database: String,
    schema: String,
    table: String,
) -> Result<Vec<crate::database::IndexInfo>> {
    let target = ConnectionTarget::new(server_id, database);
    driver()?.get_indexes(&target, &schema, &table).await
}

// ─────────────────────────────────────────────────────────────────────────────
// Connection Management
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn test_connection(server_id: i32) -> Result<bool> {
    driver()?.test_connection(server_id).await?;
    Ok(true)
}

#[tauri::command]
pub async fn get_pool_stats(
    server_id: i32,
    database: String,
) -> Result<crate::database::PoolStats> {
    let target = ConnectionTarget::new(server_id, database);
    driver()?.get_pool_stats(&target).await
}

#[tauri::command]
pub async fn disconnect_database(
    server_id: i32,
    database: String,
) -> Result<()> {
    let target = ConnectionTarget::new(server_id, database);
    driver()?.disconnect(&target).await
}

#[tauri::command]
pub async fn disconnect_server(server_id: i32) -> Result<()> {
    driver()?.disconnect_server(server_id).await
}
```

---

### `src/commands/server_commands.rs`

```rust
use rusqlite::params;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::State;

use crate::app_state::AppState;
use crate::models::Server;

type Result<T> = std::result::Result<T, String>;

#[tauri::command]
pub fn create_server(
    state: State<AppState>,
    name: String,
    host: String,
    port: i32,
    username: String,
    password: String,
    default_database: Option<String>,
) -> Result<Server> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;

    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs() as i64;

    let mut stmt = conn
        .prepare(
            r#"
            INSERT INTO servers (name, host, port, username, password, default_database, created_at) 
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) 
            RETURNING id, name, host, port, username, password, default_database, created_at
            "#,
        )
        .map_err(|e| e.to_string())?;

    stmt.query_row(
        params![name, host, port, username, password, default_database, created_at],
        |row| {
            Ok(Server {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                host: row.get(2)?,
                port: row.get(3)?,
                username: row.get(4)?,
                password: row.get(5)?,
                default_database: row.get(6)?,
                created_at: row.get(7)?,
            })
        },
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_all_servers(state: State<AppState>) -> Result<Vec<Server>> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT id, name, host, port, username, password, default_database, created_at 
             FROM servers ORDER BY id DESC",
        )
        .map_err(|e| e.to_string())?;

    let rows = stmt
        .query_map([], |row| {
            Ok(Server {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                host: row.get(2)?,
                port: row.get(3)?,
                username: row.get(4)?,
                password: row.get(5)?,
                default_database: row.get(6)?,
                created_at: row.get(7)?,
            })
        })
        .map_err(|e| e.to_string())?;

    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_server_by_id(state: State<AppState>, id: i32) -> Result<Server> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT id, name, host, port, username, password, default_database, created_at 
             FROM servers WHERE id = ?",
        )
        .map_err(|e| e.to_string())?;

    stmt.query_row(params![id], |row| {
        Ok(Server {
            id: Some(row.get(0)?),
            name: row.get(1)?,
            host: row.get(2)?,
            port: row.get(3)?,
            username: row.get(4)?,
            password: row.get(5)?,
            default_database: row.get(6)?,
            created_at: row.get(7)?,
        })
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn update_server(
    state: State<AppState>,
    id: i32,
    name: String,
    host: String,
    port: i32,
    username: String,
    password: String,
    default_database: Option<String>,
) -> Result<Server> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            r#"
            UPDATE servers 
            SET name = ?1, host = ?2, port = ?3, username = ?4, password = ?5, default_database = ?6
            WHERE id = ?7
            RETURNING id, name, host, port, username, password, default_database, created_at
            "#,
        )
        .map_err(|e| e.to_string())?;

    stmt.query_row(
        params![name, host, port, username, password, default_database, id],
        |row| {
            Ok(Server {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                host: row.get(2)?,
                port: row.get(3)?,
                username: row.get(4)?,
                password: row.get(5)?,
                default_database: row.get(6)?,
                created_at: row.get(7)?,
            })
        },
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_server(state: State<AppState>, id: i32) -> Result<()> {