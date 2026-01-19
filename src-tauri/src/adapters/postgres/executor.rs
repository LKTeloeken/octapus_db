use std::time::Instant;

use deadpool_postgres::Pool;
use tokio_postgres::Row;

use crate::error::{Error, Result};
use crate::models::{
    QueryColumnInfo, QueryOptions, QueryResult, StatementResult,
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