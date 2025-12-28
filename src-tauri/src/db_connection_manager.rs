// src/db_connection_manager.rs

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use deadpool_postgres::{
    Config, CreatePoolError, ManagerConfig, Object, Pool, RecyclingMethod, Runtime,
};
use once_cell::sync::Lazy;
use serde::Serialize;
use tokio::sync::RwLock;
use tokio_postgres::types::{FromSql, Type};
use tokio_postgres::{NoTls, Row};

use crate::app_state::AppState;
use crate::models::PostgreServer;

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
pub struct QueryResult {
    pub columns: Vec<ColumnInfo>,
    pub rows: Vec<Vec<Option<String>>>,
    pub row_count: usize,
    /// Total rows available (if count query was executed)
    pub total_count: Option<i64>,
    /// Whether more rows exist beyond what was returned
    pub has_more: bool,
}

#[derive(Debug, Serialize, Clone)]
pub struct ColumnInfo {
    pub name: String,
    pub type_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PoolStats {
    pub size: usize,
    pub available: usize,
    pub waiting: usize,
}

/// Query options for pagination and limits
#[derive(Debug, Clone, Default)]
pub struct QueryOptions {
    /// Maximum rows to return (default: 500)
    pub limit: Option<i64>,
    /// Offset for pagination
    pub offset: Option<i64>,
    /// Whether to count total rows (slower but useful for pagination UI)
    pub count_total: bool,
    /// Skip limit entirely (for exports) - use with caution
    pub unlimited: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────

/// Default row limit to prevent accidental large loads
const DEFAULT_ROW_LIMIT: i64 = 500;

// ─────────────────────────────────────────────────────────────────────────────
// Pool Manager
// ─────────────────────────────────────────────────────────────────────────────

type PoolKey = (i32, String);

static POOL_MANAGER: Lazy<Arc<RwLock<HashMap<PoolKey, Pool>>>> =
    Lazy::new(|| Arc::new(RwLock::new(HashMap::new())));

const POOL_MAX_SIZE: usize = 16;
const CONNECT_TIMEOUT_SECS: u64 = 5;

// ─────────────────────────────────────────────────────────────────────────────
// Server Config
// ─────────────────────────────────────────────────────────────────────────────

fn get_server_config(state: &AppState, id: i32) -> Result<PostgreServer> {
    let conn = state.conn.lock().map_err(|e| anyhow!("{}", e))?;

    let mut stmt = conn.prepare(
        "SELECT id, name, host, port, username, password, default_database, created_at
         FROM servers WHERE id = ?",
    )?;

    let server = stmt.query_row([id], |row| {
        Ok(PostgreServer {
            id: Some(row.get(0)?),
            name: row.get(1)?,
            host: row.get(2)?,
            port: row.get(3)?,
            username: row.get(4)?,
            password: row.get(5)?,
            default_database: row.get(6)?,
            created_at: row.get(7)?,
        })
    })?;

    Ok(server)
}

// ─────────────────────────────────────────────────────────────────────────────
// Pool Management
// ─────────────────────────────────────────────────────────────────────────────

fn create_pool(server: &PostgreServer, db_name: &str) -> Result<Pool, CreatePoolError> {
    let mut cfg = Config::new();

    cfg.host = Some(server.host.clone());
    cfg.port = Some(server.port as u16);
    cfg.user = Some(server.username.clone());
    cfg.password = Some(server.password.clone());
    cfg.dbname = Some(db_name.to_string());
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
}

pub async fn get_pool(
    state: &AppState,
    server_id: i32,
    database_name: Option<String>,
) -> Result<Pool> {
    let server_cfg = get_server_config(state, server_id)?;

    let db_name = database_name.unwrap_or_else(|| {
        server_cfg
            .default_database
            .clone()
            .unwrap_or_else(|| "postgres".to_string())
    });

    let key = (server_id, db_name.clone());

    {
        let manager = POOL_MANAGER.read().await;
        if let Some(pool) = manager.get(&key) {
            return Ok(pool.clone());
        }
    }

    let mut manager = POOL_MANAGER.write().await;

    if let Some(pool) = manager.get(&key) {
        return Ok(pool.clone());
    }

    let pool =
        create_pool(&server_cfg, &db_name).map_err(|e| anyhow!("Failed to create pool: {e}"))?;

    manager.insert(key, pool.clone());
    Ok(pool)
}

pub async fn get_connection(
    state: &AppState,
    server_id: i32,
    database_name: Option<String>,
) -> Result<Object> {
    let pool = get_pool(state, server_id, database_name).await?;
    pool.get()
        .await
        .map_err(|e| anyhow!("Failed to get connection: {e}"))
}

pub async fn test_connection(state: &AppState, server_id: i32) -> Result<()> {
    let conn = get_connection(state, server_id, None).await?;
    conn.query_one("SELECT 1", &[])
        .await
        .map_err(|e| anyhow!("Connection test failed: {e}"))?;
    Ok(())
}

pub async fn get_pool_stats(
    state: &AppState,
    server_id: i32,
    database_name: Option<String>,
) -> Result<PoolStats> {
    let pool = get_pool(state, server_id, database_name).await?;
    let status = pool.status();

    Ok(PoolStats {
        size: status.size,
        available: status.available as usize,
        waiting: status.waiting,
    })
}

pub async fn disconnect_server(server_id: i32) {
    let mut manager = POOL_MANAGER.write().await;
    manager.retain(|(sid, _), _| *sid != server_id);
}

pub async fn disconnect_database(server_id: i32, database_name: &str) {
    let mut manager = POOL_MANAGER.write().await;
    manager.remove(&(server_id, database_name.to_string()));
}

// ─────────────────────────────────────────────────────────────────────────────
// Value Extraction
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug)]
struct EnumValue(String);

impl<'a> FromSql<'a> for EnumValue {
    fn from_sql(
        _ty: &Type,
        raw: &'a [u8],
    ) -> Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        let s = std::str::from_utf8(raw)?;
        Ok(EnumValue(s.to_string()))
    }

    fn accepts(_ty: &Type) -> bool {
        true
    }
}

#[inline]
fn extract_value(row: &Row, idx: usize, pg_type: &Type) -> Option<String> {
    match pg_type.name() {
        "int2" | "smallint" => row.get::<_, Option<i16>>(idx).map(|v| v.to_string()),
        "int4" | "integer" | "serial" => row.get::<_, Option<i32>>(idx).map(|v| v.to_string()),
        "int8" | "bigint" | "bigserial" => row.get::<_, Option<i64>>(idx).map(|v| v.to_string()),
        "oid" => row.get::<_, Option<u32>>(idx).map(|v| v.to_string()),
        "float4" | "real" => row.get::<_, Option<f32>>(idx).map(|v| v.to_string()),
        "float8" | "double precision" => row.get::<_, Option<f64>>(idx).map(|v| v.to_string()),
        "bool" | "boolean" => row.get::<_, Option<bool>>(idx).map(|v| v.to_string()),
        "timestamp" => row
            .get::<_, Option<NaiveDateTime>>(idx)
            .map(|v| v.to_string()),
        "timestamptz" => row
            .get::<_, Option<DateTime<Utc>>>(idx)
            .map(|v| v.to_string()),
        "date" => row.get::<_, Option<NaiveDate>>(idx).map(|v| v.to_string()),
        "time" | "timetz" => row
            .get::<_, Option<NaiveTime>>(idx)
            .map(|v| v.to_string()),
        "bytea" => row.get::<_, Option<Vec<u8>>>(idx).map(hex::encode),
        "_int4" => row
            .get::<_, Option<Vec<i32>>>(idx)
            .map(|v| format!("{:?}", v)),
        "_int8" => row
            .get::<_, Option<Vec<i64>>>(idx)
            .map(|v| format!("{:?}", v)),
        "_text" | "_varchar" => row
            .get::<_, Option<Vec<String>>>(idx)
            .map(|v| format!("{:?}", v)),
        "text" | "varchar" | "char" | "bpchar" | "name" | "citext" | "uuid" | "json" | "jsonb"
        | "xml" | "numeric" | "decimal" | "money" | "inet" | "cidr" | "macaddr" => {
            row.get::<_, Option<String>>(idx)
        }
        // Custom types (enums, etc.)
        _ => row
            .try_get::<_, Option<String>>(idx)
            .ok()
            .flatten()
            .or_else(|| row.get::<_, Option<EnumValue>>(idx).map(|v| v.0)),
    }
}

fn process_rows(rows: &[Row]) -> (Vec<ColumnInfo>, Vec<Vec<Option<String>>>) {
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
            let mut values = Vec::with_capacity(col_count);
            for (i, pg_type) in col_types.iter().enumerate() {
                values.push(extract_value(row, i, pg_type));
            }
            values
        })
        .collect();

    (columns, result_rows)
}

// ─────────────────────────────────────────────────────────────────────────────
// Query Execution
// ─────────────────────────────────────────────────────────────────────────────

/// Execute a query with automatic pagination (default: 500 rows)
pub async fn execute_query(
    state: &AppState,
    server_id: i32,
    query: &str,
    database_name: Option<String>,
    options: Option<QueryOptions>,
) -> Result<QueryResult> {
    let opts = options.unwrap_or_default();
    let client = get_connection(state, server_id, database_name).await?;

    let trimmed = query.trim().trim_end_matches(';');

    // Determine if this is a SELECT-like query that can be paginated
    let is_select = trimmed
        .to_uppercase()
        .split_whitespace()
        .next()
        .map_or(false, |first| {
            matches!(first, "SELECT" | "TABLE" | "WITH")
        });

    // Build the actual query to execute
    let (exec_query, limit_applied) = if is_select && !opts.unlimited {
        let limit = opts.limit.unwrap_or(DEFAULT_ROW_LIMIT);
        let offset = opts.offset.unwrap_or(0);

        // Request one extra row to check if more data exists
        let query_with_limit = format!(
            "SELECT * FROM ({}) AS __q LIMIT {} OFFSET {}",
            trimmed,
            limit + 1,
            offset
        );
        (query_with_limit, Some(limit))
    } else {
        (trimmed.to_string(), None)
    };

    // Execute main query
    let rows = client
        .query(&exec_query, &[])
        .await
        .map_err(|e| anyhow!("Query error: {e}"))?;

    // Check if there are more rows
    let (rows_to_process, has_more) = if let Some(limit) = limit_applied {
        let has_more = rows.len() as i64 > limit;
        let rows_to_process = if has_more {
            &rows[..limit as usize]
        } else {
            &rows[..]
        };
        (rows_to_process, has_more)
    } else {
        (&rows[..], false)
    };

    // Get total count if requested
    let total_count = if opts.count_total && is_select {
        let count_query = format!("SELECT COUNT(*) FROM ({}) AS __count_q", trimmed);
        client
            .query_one(&count_query, &[])
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
        total_count,
        has_more,
    })
}

/// Execute a query without any limits (for exports, be careful!)
pub async fn execute_query_unlimited(
    state: &AppState,
    server_id: i32,
    query: &str,
    database_name: Option<String>,
) -> Result<QueryResult> {
    execute_query(
        state,
        server_id,
        query,
        database_name,
        Some(QueryOptions {
            unlimited: true,
            ..Default::default()
        }),
    )
    .await
}

/// Execute a statement (INSERT, UPDATE, DELETE, etc.)
pub async fn execute_statement(
    state: &AppState,
    server_id: i32,
    statement: &str,
    database_name: Option<String>,
) -> Result<u64> {
    let client = get_connection(state, server_id, database_name).await?;

    client
        .execute(statement, &[])
        .await
        .map_err(|e| anyhow!("Statement error: {e}"))
}

/// Execute multiple statements in a transaction
pub async fn execute_transaction(
    state: &AppState,
    server_id: i32,
    statements: Vec<String>,
    database_name: Option<String>,
) -> Result<Vec<u64>> {
    let mut client = get_connection(state, server_id, database_name).await?;

    let tx = client
        .transaction()
        .await
        .map_err(|e| anyhow!("Failed to start transaction: {e}"))?;

    let mut results = Vec::with_capacity(statements.len());

    for stmt in &statements {
        let rows = tx
            .execute(stmt.as_str(), &[])
            .await
            .map_err(|e| anyhow!("Transaction error: {e}"))?;
        results.push(rows);
    }

    tx.commit()
        .await
        .map_err(|e| anyhow!("Failed to commit: {e}"))?;

    Ok(results)
}