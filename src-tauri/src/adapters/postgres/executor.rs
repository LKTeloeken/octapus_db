use std::collections::HashSet;
use std::time::Instant;

use deadpool_postgres::Pool;
use tokio_postgres::Row;

use crate::error::{Error, Result};
use crate::models::{
    EditableInfo, QueryColumnInfo, QueryOptions, QueryResult, StatementResult,
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

    // Detect if the result is editable (single source table with a primary key)
    let editable_info = if is_select && !columns.is_empty() {
        detect_editable_info(&client, trimmed, &columns).await
    } else {
        None
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

/// Detects if the query result is editable by checking:
/// 1. All columns come from the same source table (via table_oid)
/// 2. The source table has a primary key
/// 3. All PK columns are present in the result
///
/// Uses `prepare()` on the original (unwrapped) query to get accurate
/// table_oid metadata, since the pagination wrapper loses this info.
async fn detect_editable_info(
    client: &deadpool_postgres::Object,
    original_query: &str,
    result_columns: &[QueryColumnInfo],
) -> Option<EditableInfo> {
    let stmt = client.prepare(original_query).await.ok()?;
    let stmt_columns = stmt.columns();

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