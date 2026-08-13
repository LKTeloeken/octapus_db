use std::collections::HashSet;
use std::time::Instant;

use deadpool_postgres::Pool;
use tokio_postgres::{Column, Row, SimpleQueryMessage, SimpleQueryRow, Statement};

use super::QueryRegistry;

use crate::error::{Error, Result};
use crate::models::{
    EditableInfo, QueryColumnInfo, QueryOptions, QueryResult, RowEdit, RowInsert, StatementResult,
};

use super::util::{get_column_types, quote_ident};

/// Removes a query_id → backend PID entry when the execution finishes,
/// including early returns and cancelled/failed queries.
struct PidGuard<'a> {
    registry: &'a QueryRegistry,
    query_id: String,
}

impl Drop for PidGuard<'_> {
    fn drop(&mut self) {
        self.registry.lock().unwrap().remove(&self.query_id);
    }
}

pub async fn execute_query(
    pool: &Pool,
    query: &str,
    options: QueryOptions,
    registry: &QueryRegistry,
) -> Result<QueryResult> {
    let client = pool.get().await?;
    let trimmed = query.trim().trim_end_matches(';');

    if trimmed.is_empty() {
        return Err(Error::InvalidQuery("Empty query".into()));
    }

    // Map the query_id to this connection's backend PID so cancel_query can
    // target it with pg_cancel_backend from another connection.
    let _pid_guard = match options.query_id.as_deref() {
        Some(query_id) => {
            let pid: i32 = client
                .query_one("SELECT pg_backend_pid()", &[])
                .await?
                .get(0);
            registry.lock().unwrap().insert(query_id.to_string(), pid);
            Some(PidGuard {
                registry,
                query_id: query_id.to_string(),
            })
        }
        None => None,
    };

    let is_select = is_select_query(trimmed);

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

    // Run three operations pipelined on the same connection so their network
    // round-trips overlap instead of stacking up:
    //   - prepare(): column metadata (names + original types) + table OIDs,
    //     reused for the result columns and for editability detection;
    //   - simple_query(): the actual data. Postgres renders every value as text,
    //     so jsonb/numeric/uuid/arrays/enums all come back correctly and the
    //     backend never decodes per type;
    //   - COUNT(*): the optional total, which no longer blocks the data fetch.
    // Only the data query is timed, so `executionTimeMs` reflects execution+fetch
    // (comparable to other clients) — not the extra prepare round-trip nor the
    // count scan. `prepare` only accepts a single statement (None for scripts).
    let exec_started = Instant::now();
    let data_fut = async {
        let messages = client.simple_query(&exec_query).await;
        (messages, exec_started.elapsed())
    };
    let count_fut = async {
        if options.count_total && is_select {
            let count_query = format!("SELECT COUNT(*) FROM ({trimmed}) AS __c");
            client
                .query_one(&count_query, &[])
                .await
                .ok()
                .and_then(|r| r.get::<_, Option<i64>>(0))
        } else {
            None
        }
    };
    let (stmt, (messages, exec_elapsed), total_count) =
        tokio::join!(client.prepare(trimmed), data_fut, count_fut);

    let stmt = stmt.ok();
    let messages = messages?;
    let execution_time_ms = exec_elapsed.as_millis() as u64;

    let data_rows: Vec<&SimpleQueryRow> = messages
        .iter()
        .filter_map(|m| match m {
            SimpleQueryMessage::Row(row) => Some(row),
            _ => None,
        })
        .collect();

    // Determine if more rows exist
    let (rows_to_process, has_more) = match limit {
        Some(l) if data_rows.len() as i64 > l => (&data_rows[..l as usize], true),
        _ => (&data_rows[..], false),
    };

    // Column metadata: prefer the prepared statement (real types, present even
    // for an empty result set); fall back to the simple-query row description.
    let columns = match &stmt {
        Some(stmt) => columns_from_statement(stmt),
        None => rows_to_process
            .first()
            .map(|row| columns_from_simple(row))
            .unwrap_or_default(),
    };

    let result_rows = extract_text_rows(rows_to_process);

    // Detect if the result is editable (single source table with a primary key)
    let editable_info = match &stmt {
        Some(stmt) if is_select && !columns.is_empty() => {
            detect_editable_info(&client, stmt.columns(), &columns).await
        }
        _ => None,
    };

    Ok(QueryResult {
        columns,
        rows: result_rows,
        row_count: rows_to_process.len(),
        total_count,
        has_more,
        execution_time_ms,
        editable_info,
    })
}

pub async fn apply_row_edits(
    pool: &Pool,
    editable: &EditableInfo,
    edits: Vec<RowEdit>,
) -> Result<StatementResult> {
    if edits.is_empty() {
        return Ok(StatementResult {
            affected_rows: 0,
            execution_time_ms: 0,
        });
    }

    let mut client = pool.get().await?;

    let type_map =
        get_column_types(&client, &editable.schema, &editable.table).await?;

    let start = Instant::now();
    let tx = client.transaction().await?;
    let mut total_affected: u64 = 0;

    for edit in &edits {
        if edit.changes.is_empty() {
            continue;
        }

        if edit.pk_values.len() != editable.primary_key_columns.len() {
            return Err(Error::InvalidQuery(
                "Primary key value count does not match primary key columns"
                    .into(),
            ));
        }

        let mut param_idx: usize = 1;
        let mut set_clauses: Vec<String> = Vec::new();
        let mut where_clauses: Vec<String> = Vec::new();
        let mut param_values: Vec<Option<&str>> = Vec::new();

        // ── SET clause ───────────────────────────────────────────────
        for (col_name, value) in &edit.changes {
            let col_type = type_map.get(col_name.as_str()).ok_or_else(|| {
                Error::InvalidQuery(format!("Unknown column: {col_name}"))
            })?;

            set_clauses.push(format!(
                "{} = ${}::text::{}",
                quote_ident(col_name),
                param_idx,
                col_type
            ));
            param_values.push(value.as_deref());
            param_idx += 1;
        }

        // ── WHERE clause (identify row by PK) ───────────────────────
        for (pk_col, pk_val) in
            editable.primary_key_columns.iter().zip(&edit.pk_values)
        {
            let col_type = type_map.get(pk_col.as_str()).ok_or_else(|| {
                Error::InvalidQuery(format!(
                    "Unknown primary key column: {pk_col}"
                ))
            })?;

            where_clauses.push(format!(
                "{} = ${}::text::{}",
                quote_ident(pk_col),
                param_idx,
                col_type
            ));
            param_values.push(pk_val.as_deref());
            param_idx += 1;
        }

        let query = format!(
            "UPDATE {}.{} SET {} WHERE {}",
            quote_ident(&editable.schema),
            quote_ident(&editable.table),
            set_clauses.join(", "),
            where_clauses.join(" AND "),
        );

        let params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
            param_values
                .iter()
                .map(|v| v as &(dyn tokio_postgres::types::ToSql + Sync))
                .collect();

        let affected = tx.execute(&query, &params).await.map_err(|e| {
            Error::Query(format!("Failed to update row: {e}"))
        })?;

        total_affected += affected;
    }

    tx.commit().await?;

    Ok(StatementResult {
        affected_rows: total_affected,
        execution_time_ms: start.elapsed().as_millis() as u64,
    })
}

pub async fn insert_rows(
    pool: &Pool,
    editable: &EditableInfo,
    rows: Vec<RowInsert>,
) -> Result<StatementResult> {
    if rows.is_empty() {
        return Ok(StatementResult {
            affected_rows: 0,
            execution_time_ms: 0,
        });
    }

    let mut client = pool.get().await?;

    let type_map =
        get_column_types(&client, &editable.schema, &editable.table).await?;

    let start = Instant::now();
    let tx = client.transaction().await?;
    let mut total_affected: u64 = 0;

    for row in &rows {
        // Only the filled columns are sent; omitted ones fall back to the
        // database default (serial, DEFAULT, generated identity).
        if row.values.is_empty() {
            continue;
        }

        let mut columns: Vec<String> = Vec::new();
        let mut placeholders: Vec<String> = Vec::new();
        let mut param_values: Vec<Option<&str>> = Vec::new();

        for (i, (col_name, value)) in row.values.iter().enumerate() {
            let col_type = type_map.get(col_name.as_str()).ok_or_else(|| {
                Error::InvalidQuery(format!("Unknown column: {col_name}"))
            })?;

            columns.push(quote_ident(col_name));
            placeholders.push(format!("${}::text::{}", i + 1, col_type));
            param_values.push(value.as_deref());
        }

        let query = format!(
            "INSERT INTO {}.{} ({}) VALUES ({})",
            quote_ident(&editable.schema),
            quote_ident(&editable.table),
            columns.join(", "),
            placeholders.join(", "),
        );

        let params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
            param_values
                .iter()
                .map(|v| v as &(dyn tokio_postgres::types::ToSql + Sync))
                .collect();

        let affected = tx.execute(&query, &params).await.map_err(|e| {
            Error::Query(format!("Failed to insert row: {e}"))
        })?;

        total_affected += affected;
    }

    tx.commit().await?;

    Ok(StatementResult {
        affected_rows: total_affected,
        execution_time_ms: start.elapsed().as_millis() as u64,
    })
}

pub async fn delete_rows(
    pool: &Pool,
    editable: &EditableInfo,
    pk_values: Vec<Vec<Option<String>>>,
) -> Result<StatementResult> {
    if pk_values.is_empty() {
        return Ok(StatementResult {
            affected_rows: 0,
            execution_time_ms: 0,
        });
    }

    let mut client = pool.get().await?;

    let type_map =
        get_column_types(&client, &editable.schema, &editable.table).await?;

    let start = Instant::now();
    let tx = client.transaction().await?;
    let mut total_affected: u64 = 0;

    for pk in &pk_values {
        if pk.len() != editable.primary_key_columns.len() {
            return Err(Error::InvalidQuery(
                "Primary key value count does not match primary key columns"
                    .into(),
            ));
        }

        let mut where_clauses: Vec<String> = Vec::new();
        let mut param_values: Vec<Option<&str>> = Vec::new();

        for (i, (pk_col, pk_val)) in
            editable.primary_key_columns.iter().zip(pk).enumerate()
        {
            let col_type = type_map.get(pk_col.as_str()).ok_or_else(|| {
                Error::InvalidQuery(format!(
                    "Unknown primary key column: {pk_col}"
                ))
            })?;

            where_clauses.push(format!(
                "{} = ${}::text::{}",
                quote_ident(pk_col),
                i + 1,
                col_type
            ));
            param_values.push(pk_val.as_deref());
        }

        let query = format!(
            "DELETE FROM {}.{} WHERE {}",
            quote_ident(&editable.schema),
            quote_ident(&editable.table),
            where_clauses.join(" AND "),
        );

        let params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
            param_values
                .iter()
                .map(|v| v as &(dyn tokio_postgres::types::ToSql + Sync))
                .collect();

        let affected = tx.execute(&query, &params).await.map_err(|e| {
            Error::Query(format!("Failed to delete row: {e}"))
        })?;

        total_affected += affected;
    }

    tx.commit().await?;

    Ok(StatementResult {
        affected_rows: total_affected,
        execution_time_ms: start.elapsed().as_millis() as u64,
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

/// Builds column metadata from a prepared statement (names + original types).
fn columns_from_statement(stmt: &Statement) -> Vec<QueryColumnInfo> {
    stmt.columns()
        .iter()
        .map(|c| QueryColumnInfo {
            name: c.name().to_string(),
            type_name: c.type_().name().to_string(),
            type_oid: Some(c.type_().oid()),
        })
        .collect()
}

/// Fallback column metadata from a simple-query row (no types available).
fn columns_from_simple(row: &SimpleQueryRow) -> Vec<QueryColumnInfo> {
    row.columns()
        .iter()
        .map(|c| QueryColumnInfo {
            name: c.name().to_string(),
            type_name: String::new(),
            type_oid: None,
        })
        .collect()
}

/// Extracts the text values of simple-query rows (Postgres already rendered
/// every value as text, so no per-type decoding happens here).
fn extract_text_rows(rows: &[&SimpleQueryRow]) -> Vec<Vec<Option<String>>> {
    rows.iter()
        .map(|row| (0..row.len()).map(|i| row.get(i).map(str::to_string)).collect())
        .collect()
}

/// Extracts the text values of extended-protocol rows whose columns were all
/// cast to `::text` in the SQL (used by the table browser). No type decoding.
pub(super) fn extract_text_rows_typed(rows: &[Row]) -> Vec<Vec<Option<String>>> {
    rows.iter()
        .map(|row| (0..row.len()).map(|i| row.get::<_, Option<String>>(i)).collect())
        .collect()
}

/// Detects if the query result is editable by checking:
/// 1. All columns come from the same source table (via table_oid)
/// 2. The source table has a primary key
/// 3. All PK columns are present in the result
///
/// Takes the columns of the prepared (unwrapped) statement to read accurate
/// table_oid metadata, since the pagination wrapper loses this info.
async fn detect_editable_info(
    client: &deadpool_postgres::Object,
    stmt_columns: &[Column],
    result_columns: &[QueryColumnInfo],
) -> Option<EditableInfo> {
    // Collect unique non-zero table OIDs from the columns
    let table_oids: HashSet<u32> = stmt_columns
        .iter()
        .filter_map(|c| c.table_oid())
        .filter(|&oid| oid != 0)
        .collect();

    // All real columns must come from exactly one table
    if table_oids.len() != 1 {
        return None;
    }

    let table_oid = *table_oids.iter().next()?;

    // Resolve table OID to schema + table name
    let table_row = client
        .query_one(
            "SELECT n.nspname, c.relname \
             FROM pg_class c \
             JOIN pg_namespace n ON n.oid = c.relnamespace \
             WHERE c.oid = $1",
            &[&table_oid],
        )
        .await
        .ok()?;

    let schema: String = table_row.get(0);
    let table: String = table_row.get(1);

    // Get primary key column names for this table
    let pk_rows = client
        .query(
            "SELECT a.attname \
             FROM pg_constraint con \
             JOIN pg_attribute a \
               ON a.attrelid = con.conrelid AND a.attnum = ANY(con.conkey) \
             WHERE con.conrelid = $1 AND con.contype = 'p' \
             ORDER BY array_position(con.conkey, a.attnum)",
            &[&table_oid],
        )
        .await
        .ok()?;

    if pk_rows.is_empty() {
        return None;
    }

    let pk_column_names: Vec<String> = pk_rows.iter().map(|r| r.get(0)).collect();

    // Find PK column indices in the result set
    let pk_indices: Vec<usize> = pk_column_names
        .iter()
        .filter_map(|pk_name| {
            result_columns.iter().position(|c| c.name == *pk_name)
        })
        .collect();

    // All PK columns must be present in the result
    if pk_indices.len() != pk_column_names.len() {
        return None;
    }

    Some(EditableInfo {
        schema,
        table,
        primary_key_columns: pk_column_names,
        primary_key_column_indices: pk_indices,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── End-to-end (requires a local Postgres; run with `cargo test -- --ignored`) ──

    #[tokio::test]
    #[ignore = "requires local postgres on localhost:5432 (postgres/1234)"]
    async fn e2e_cancel_query() {
        use std::sync::Arc;
        use std::time::Duration;

        use crate::adapters::postgres::PostgresAdapter;
        use crate::adapters::DatabaseAdapter;
        use crate::models::{DatabaseType, QueryOptions, Server};

        let server = Server {
            id: Some(1),
            name: "e2e".into(),
            db_type: DatabaseType::Postgres,
            host: "localhost".into(),
            port: 5432,
            username: "postgres".into(),
            password: "1234".into(),
            default_database: None,
            ssl_enabled: false,
            connection_uri: None,
            created_at: 0,
        };

        let adapter = Arc::new(PostgresAdapter::new(&server, "postgres").unwrap());
        let query_id = "e2e-cancel";

        let runner = adapter.clone();
        let handle = tokio::spawn(async move {
            runner
                .execute_query(
                    "SELECT pg_sleep(30)",
                    QueryOptions {
                        query_id: Some(query_id.into()),
                        ..Default::default()
                    },
                )
                .await
        });

        // Wait for the backend PID to be registered before cancelling
        let mut registered = false;
        for _ in 0..50 {
            if adapter.active_queries.lock().unwrap().contains_key(query_id) {
                registered = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        assert!(registered, "query_id was never registered");

        adapter.cancel_query(query_id).await.unwrap();

        let err = handle.await.unwrap().unwrap_err();
        assert!(
            err.to_string().contains("canceling statement"),
            "expected cancellation error, got: {err}"
        );

        // The guard must clean the registry once the query errors out
        assert!(!adapter.active_queries.lock().unwrap().contains_key(query_id));

        // Cancelling a finished/unknown id is a no-op
        adapter.cancel_query(query_id).await.unwrap();
    }

    /// Regression: rich types (jsonb/json/numeric/uuid/arrays/bool/timestamptz)
    /// used to come back as NULL because tokio-postgres can't decode them as
    /// String. Now Postgres renders everything as text, so every column must
    /// return its value through both the free editor and the table browser.
    #[tokio::test]
    #[ignore = "requires local postgres on localhost:5432 (postgres/1234)"]
    async fn e2e_rich_types_returned_as_text() {
        use crate::adapters::postgres::PostgresAdapter;
        use crate::adapters::DatabaseAdapter;
        use crate::models::{DatabaseType, QueryOptions, Server, TableDataRequest};

        let server = Server {
            id: Some(1),
            name: "e2e".into(),
            db_type: DatabaseType::Postgres,
            host: "localhost".into(),
            port: 5432,
            username: "postgres".into(),
            password: "1234".into(),
            default_database: None,
            ssl_enabled: false,
            connection_uri: None,
            created_at: 0,
        };

        let admin = PostgresAdapter::new(&server, "postgres").unwrap();
        admin
            .execute_statement("DROP DATABASE IF EXISTS octapus_db_e2e_types WITH (FORCE)")
            .await
            .unwrap();
        admin
            .execute_statement("CREATE DATABASE octapus_db_e2e_types")
            .await
            .unwrap();

        let adapter = PostgresAdapter::new(&server, "octapus_db_e2e_types").unwrap();
        adapter
            .execute_statement(
                "CREATE TABLE rich (\
                   id serial PRIMARY KEY, \
                   data jsonb, \
                   doc json, \
                   price numeric(10,2), \
                   uid uuid, \
                   tags int[], \
                   flag boolean, \
                   ts timestamptz)",
            )
            .await
            .unwrap();
        adapter
            .execute_statement(
                "INSERT INTO rich (data, doc, price, uid, tags, flag, ts) VALUES (\
                   '{\"k\": 1}'::jsonb, \
                   '{\"k\": 2}'::json, \
                   19.90, \
                   '11111111-1111-1111-1111-111111111111'::uuid, \
                   '{1,2,3}'::int[], \
                   true, \
                   '2024-01-15 10:30:00+00'::timestamptz)",
            )
            .await
            .unwrap();

        // Asserts every non-id column of the single row is present (not NULL).
        let assert_all_present = |columns: &[QueryColumnInfo], row: &[Option<String>]| {
            for (col, cell) in columns.iter().zip(row.iter()) {
                if col.name == "id" {
                    continue;
                }
                assert!(
                    cell.as_deref().map(|s| !s.is_empty()).unwrap_or(false),
                    "column {} ({}) came back empty/NULL",
                    col.name,
                    col.type_name
                );
            }
        };

        // 1) Free editor path (simple_query / prepared metadata)
        let result = adapter
            .execute_query("SELECT * FROM rich ORDER BY id", QueryOptions::default())
            .await
            .unwrap();
        assert_eq!(result.row_count, 1);
        // Metadata keeps the original Postgres type names (front editor picks).
        assert!(result.columns.iter().any(|c| c.type_name == "jsonb"));
        assert_all_present(&result.columns, &result.rows[0]);

        // 2) Table browser path (extended protocol + ::text casts)
        let browsed = adapter
            .fetch_table_data(TableDataRequest {
                schema: None,
                table: "rich".into(),
                filters: vec![],
                sort: vec![],
                limit: 50,
                offset: 0,
                count_total: false,
            })
            .await
            .unwrap();
        assert_eq!(browsed.row_count, 1);
        assert!(browsed.columns.iter().any(|c| c.type_name == "jsonb"));
        assert_all_present(&browsed.columns, &browsed.rows[0]);

        drop(adapter);
        admin
            .execute_statement("DROP DATABASE octapus_db_e2e_types WITH (FORCE)")
            .await
            .unwrap();
    }
}