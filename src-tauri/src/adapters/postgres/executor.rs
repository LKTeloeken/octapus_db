use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use deadpool_postgres::Pool;
use parking_lot::Mutex;
use tokio_postgres::Row;

use crate::error::{Error, Result};
use crate::models::{
    EditableInfo, ForeignKeyTarget, QueryColumnInfo, QueryOptions, QueryResult, RowEdit,
    StatementResult,
};

use super::types::extract_value;

struct ActiveQueryGuard<'a> {
    active_query_pid: &'a Arc<Mutex<Option<i32>>>,
}

impl<'a> ActiveQueryGuard<'a> {
    fn new(active_query_pid: &'a Arc<Mutex<Option<i32>>>) -> Self {
        Self { active_query_pid }
    }
}

impl Drop for ActiveQueryGuard<'_> {
    fn drop(&mut self) {
        *self.active_query_pid.lock() = None;
    }
}

pub async fn execute_query(
    pool: &Pool,
    query: &str,
    options: QueryOptions,
    active_query_pid: Arc<Mutex<Option<i32>>>,
) -> Result<QueryResult> {
    let client = pool.get().await?;
    let trimmed = query.trim().trim_end_matches(';');

    if trimmed.is_empty() {
        return Err(Error::InvalidQuery("Empty query".into()));
    }

    let is_select = is_select_query(trimmed);
    let query_pid: i32 = client
        .query_one("SELECT pg_backend_pid()", &[])
        .await?
        .get(0);
    *active_query_pid.lock() = Some(query_pid);
    let _query_guard = ActiveQueryGuard::new(&active_query_pid);

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

    let (mut columns, result_rows) = process_rows(rows_to_process);

    // Detect if the result is editable (single source table with a primary key)
    let editable_info = if is_select && !columns.is_empty() {
        detect_editable_info(&client, trimmed, &columns).await
    } else {
        None
    };

    if let Some(editable) = &editable_info {
        let foreign_key_columns =
            get_foreign_key_targets(&client, &editable.schema, &editable.table).await?;

        columns.iter_mut().for_each(|column| {
            if let Some(target) = foreign_key_columns.get(&column.name) {
                column.foreign_key_target = Some(target.clone());
            }
        });
    }

    Ok(QueryResult {
        columns,
        rows: result_rows,
        row_count: rows_to_process.len(),
        total_count,
        has_more,
        execution_time_ms,
        editable_info,
        query_id: Some(query_pid.to_string()),
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

pub async fn insert_table_rows(
    pool: &Pool,
    editable: &EditableInfo,
    column_names: Vec<String>,
    rows: Vec<Vec<Option<String>>>,
) -> Result<StatementResult> {
    if rows.is_empty() {
        return Ok(StatementResult {
            affected_rows: 0,
            execution_time_ms: 0,
        });
    }

    if column_names.is_empty() {
        return Err(Error::InvalidQuery("No columns provided for insert".into()));
    }

    let mut client = pool.get().await?;
    let type_map = get_column_types(&client, &editable.schema, &editable.table).await?;
    let start = Instant::now();
    let tx = client.transaction().await?;
    let mut total_affected: u64 = 0;

    for row_values in rows {
        if row_values.len() != column_names.len() {
            return Err(Error::InvalidQuery(
                "Inserted row value count does not match insert columns".into(),
            ));
        }

        let mut value_clauses: Vec<String> = Vec::with_capacity(column_names.len());
        let mut param_values: Vec<Option<String>> = Vec::with_capacity(column_names.len());

        for (index, (column_name, value)) in
            column_names.iter().zip(row_values.iter()).enumerate()
        {
            let col_type = type_map
                .get(column_name.as_str())
                .ok_or_else(|| Error::InvalidQuery(format!("Unknown column: {column_name}")))?;
            value_clauses.push(format!("${}::text::{}", index + 1, col_type));
            param_values.push(value.clone());
        }

        let insert_query = format!(
            "INSERT INTO {}.{} ({}) VALUES ({})",
            quote_ident(&editable.schema),
            quote_ident(&editable.table),
            column_names
                .iter()
                .map(|name| quote_ident(name))
                .collect::<Vec<_>>()
                .join(", "),
            value_clauses.join(", "),
        );

        let params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = param_values
            .iter()
            .map(|v| v as &(dyn tokio_postgres::types::ToSql + Sync))
            .collect();

        let affected = tx
            .execute(&insert_query, &params)
            .await
            .map_err(|e| Error::Query(format!("Failed to insert row: {e}")))?;
        total_affected += affected;
    }

    tx.commit().await?;

    Ok(StatementResult {
        affected_rows: total_affected,
        execution_time_ms: start.elapsed().as_millis() as u64,
    })
}

pub async fn delete_table_rows(
    pool: &Pool,
    editable: &EditableInfo,
    pk_values_list: Vec<Vec<Option<String>>>,
) -> Result<StatementResult> {
    if pk_values_list.is_empty() {
        return Ok(StatementResult {
            affected_rows: 0,
            execution_time_ms: 0,
        });
    }

    let mut client = pool.get().await?;
    let type_map = get_column_types(&client, &editable.schema, &editable.table).await?;
    let start = Instant::now();
    let tx = client.transaction().await?;
    let mut total_affected: u64 = 0;

    for pk_values in pk_values_list {
        if pk_values.len() != editable.primary_key_columns.len() {
            return Err(Error::InvalidQuery(
                "Primary key value count does not match primary key columns".into(),
            ));
        }

        let mut where_clauses: Vec<String> = Vec::with_capacity(pk_values.len());
        let mut param_values: Vec<Option<String>> = Vec::with_capacity(pk_values.len());

        for (index, (pk_col, pk_val)) in editable
            .primary_key_columns
            .iter()
            .zip(pk_values.iter())
            .enumerate()
        {
            let col_type = type_map
                .get(pk_col.as_str())
                .ok_or_else(|| Error::InvalidQuery(format!("Unknown primary key column: {pk_col}")))?;
            where_clauses.push(format!("{} = ${}::text::{}", quote_ident(pk_col), index + 1, col_type));
            param_values.push(pk_val.clone());
        }

        let delete_query = format!(
            "DELETE FROM {}.{} WHERE {}",
            quote_ident(&editable.schema),
            quote_ident(&editable.table),
            where_clauses.join(" AND "),
        );

        let params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = param_values
            .iter()
            .map(|v| v as &(dyn tokio_postgres::types::ToSql + Sync))
            .collect();

        let affected = tx
            .execute(&delete_query, &params)
            .await
            .map_err(|e| Error::Query(format!("Failed to delete row: {e}")))?;
        total_affected += affected;
    }

    tx.commit().await?;

    Ok(StatementResult {
        affected_rows: total_affected,
        execution_time_ms: start.elapsed().as_millis() as u64,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Internal helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Fetches a mapping of `column_name -> pg_type_name` for a given table.
async fn get_column_types(
    client: &deadpool_postgres::Object,
    schema: &str,
    table: &str,
) -> Result<HashMap<String, String>> {
    let rows = client
        .query(
            "SELECT a.attname::text, format_type(a.atttypid, a.atttypmod) \
             FROM pg_attribute a \
             JOIN pg_class c ON c.oid = a.attrelid \
             JOIN pg_namespace n ON n.oid = c.relnamespace \
             WHERE n.nspname = $1 \
               AND c.relname = $2 \
               AND a.attnum > 0 \
               AND NOT a.attisdropped",
            &[&schema, &table],
        )
        .await
        .map_err(|e| {
            Error::Query(format!("Failed to fetch column types: {e}"))
        })?;

    let map: HashMap<String, String> = rows
        .iter()
        .map(|r| {
            let name: String = r.get(0);
            let typ: String = r.get(1);
            (name, typ)
        })
        .collect();

    if map.is_empty() {
        return Err(Error::InvalidQuery(format!(
            "Table {schema}.{table} not found or has no columns"
        )));
    }

    Ok(map)
}

/// Safely quotes a PostgreSQL identifier (prevents SQL injection on names).
fn quote_ident(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

async fn get_foreign_key_targets(
    client: &deadpool_postgres::Object,
    schema: &str,
    table: &str,
) -> Result<HashMap<String, ForeignKeyTarget>> {
    let rows = client
        .query(
            r#"
            SELECT
                src.attname as source_column,
                target_ns.nspname as target_schema,
                target_tbl.relname as target_table,
                target_col.attname as target_column
            FROM pg_constraint fk
            JOIN pg_class source_tbl ON source_tbl.oid = fk.conrelid
            JOIN pg_namespace source_ns ON source_ns.oid = source_tbl.relnamespace
            JOIN pg_class target_tbl ON target_tbl.oid = fk.confrelid
            JOIN pg_namespace target_ns ON target_ns.oid = target_tbl.relnamespace
            JOIN LATERAL unnest(fk.conkey) WITH ORDINALITY AS src_key(attnum, ord) ON true
            JOIN LATERAL unnest(fk.confkey) WITH ORDINALITY AS ref_key(attnum, ord) ON ref_key.ord = src_key.ord
            JOIN pg_attribute src ON src.attrelid = source_tbl.oid AND src.attnum = src_key.attnum
            JOIN pg_attribute target_col ON target_col.attrelid = target_tbl.oid AND target_col.attnum = ref_key.attnum
            WHERE fk.contype = 'f'
              AND source_ns.nspname = $1
              AND source_tbl.relname = $2
            "#,
            &[&schema, &table],
        )
        .await?;

    Ok(rows
        .iter()
        .map(|row| {
            let source_column: String = row.get(0);
            let target = ForeignKeyTarget {
                schema: row.get(1),
                table: row.get(2),
                column: row.get(3),
            };
            (source_column, target)
        })
        .collect())
}

pub async fn cancel_query(
    pool: &Pool,
    query_id: &str,
    active_query_pid: Arc<Mutex<Option<i32>>>,
) -> Result<()> {
    let fallback_pid = query_id.parse::<i32>().ok();
    let pid = (*active_query_pid.lock()).or(fallback_pid).ok_or_else(|| {
        Error::InvalidState("No active query to cancel".into())
    })?;

    let client = pool.get().await?;
    let canceled: bool = client
        .query_one("SELECT pg_cancel_backend($1)", &[&pid])
        .await?
        .get(0);

    if canceled {
        Ok(())
    } else {
        Err(Error::InvalidState("Failed to cancel active query".into()))
    }
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
            foreign_key_target: None,
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
