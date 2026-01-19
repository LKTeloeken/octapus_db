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