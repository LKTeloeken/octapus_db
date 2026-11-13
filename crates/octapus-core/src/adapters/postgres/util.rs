use std::collections::HashMap;

use crate::error::{Error, Result};

/// Safely quotes a PostgreSQL identifier (prevents SQL injection on names).
pub fn quote_ident(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

/// Metadata for a single table column, in physical (`attnum`) order.
pub struct ColumnMeta {
    pub name: String,
    /// Internal Postgres type name (e.g. `int4`, `jsonb`, `timestamptz`) — this
    /// is what the frontend uses to pick the right cell editor.
    pub type_name: String,
    /// `format_type()` output (e.g. `integer`, `jsonb`, `timestamp with time
    /// zone`) — used to build `$N::text::<type>` casts on edits.
    pub format_type: String,
    pub type_oid: u32,
}

/// Fetches the ordered column metadata for a given table.
pub async fn get_columns_ordered(
    client: &deadpool_postgres::Object,
    schema: &str,
    table: &str,
) -> Result<Vec<ColumnMeta>> {
    let rows = client
        .query(
            "SELECT a.attname::text, t.typname::text, \
                    format_type(a.atttypid, a.atttypmod), a.atttypid \
             FROM pg_attribute a \
             JOIN pg_class c ON c.oid = a.attrelid \
             JOIN pg_namespace n ON n.oid = c.relnamespace \
             JOIN pg_type t ON t.oid = a.atttypid \
             WHERE n.nspname = $1 \
               AND c.relname = $2 \
               AND a.attnum > 0 \
               AND NOT a.attisdropped \
             ORDER BY a.attnum",
            &[&schema, &table],
        )
        .await
        .map_err(|e| {
            Error::Query(format!("Failed to fetch column types: {e}"))
        })?;

    let columns: Vec<ColumnMeta> = rows
        .iter()
        .map(|r| ColumnMeta {
            name: r.get(0),
            type_name: r.get(1),
            format_type: r.get(2),
            type_oid: r.get(3),
        })
        .collect();

    if columns.is_empty() {
        return Err(Error::InvalidQuery(format!(
            "Table {schema}.{table} not found or has no columns"
        )));
    }

    Ok(columns)
}

/// Fetches a mapping of `column_name -> format_type` for a given table
/// (used for type validation and `::text::<type>` casts).
pub async fn get_column_types(
    client: &deadpool_postgres::Object,
    schema: &str,
    table: &str,
) -> Result<HashMap<String, String>> {
    Ok(get_columns_ordered(client, schema, table)
        .await?
        .into_iter()
        .map(|c| (c.name, c.format_type))
        .collect())
}
