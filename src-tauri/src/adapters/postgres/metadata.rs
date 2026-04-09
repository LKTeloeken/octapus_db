use std::collections::BTreeMap;
use chrono::Utc;

use deadpool_postgres::Pool;

use crate::error::Result;
use crate::models::{
    ColumnInfo, DatabaseInfo, IndexInfo, SchemaInfo, TableInfo, TableType, DatabaseStructure, SchemaStructure,
    TableStructure, ForeignKeyTarget,
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
                ) as is_fk,
                (
                    SELECT fn.nspname
                    FROM pg_constraint c
                    JOIN pg_class fc ON fc.oid = c.confrelid
                    JOIN pg_namespace fn ON fn.oid = fc.relnamespace
                    JOIN LATERAL unnest(c.conkey) WITH ORDINALITY AS src(attnum, ord) ON true
                    WHERE c.conrelid = a.attrelid
                      AND c.contype = 'f'
                      AND src.attnum = a.attnum
                    LIMIT 1
                ) as fk_schema,
                (
                    SELECT fc.relname
                    FROM pg_constraint c
                    JOIN pg_class fc ON fc.oid = c.confrelid
                    JOIN LATERAL unnest(c.conkey) WITH ORDINALITY AS src(attnum, ord) ON true
                    WHERE c.conrelid = a.attrelid
                      AND c.contype = 'f'
                      AND src.attnum = a.attnum
                    LIMIT 1
                ) as fk_table,
                (
                    SELECT fa.attname
                    FROM pg_constraint c
                    JOIN pg_class fc ON fc.oid = c.confrelid
                    JOIN LATERAL unnest(c.conkey) WITH ORDINALITY AS src(attnum, ord) ON true
                    JOIN LATERAL unnest(c.confkey) WITH ORDINALITY AS ref(attnum, ord) ON ref.ord = src.ord
                    JOIN pg_attribute fa ON fa.attrelid = fc.oid AND fa.attnum = ref.attnum
                    WHERE c.conrelid = a.attrelid
                      AND c.contype = 'f'
                      AND src.attnum = a.attnum
                    LIMIT 1
                ) as fk_column
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
            foreign_key_target: match (
                r.get::<_, Option<String>>(7),
                r.get::<_, Option<String>>(8),
                r.get::<_, Option<String>>(9),
            ) {
                (Some(schema), Some(table), Some(column)) => Some(ForeignKeyTarget {
                    schema,
                    table,
                    column,
                }),
                _ => None,
            },
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

pub async fn list_schemas_with_tables(pool: &Pool) -> Result<DatabaseStructure> {
    let client = pool.get().await?;

    let rows = client
        .query(
            r#"
            SELECT
                n.nspname AS schema_name,
                c.relname AS table_name,
                c.relkind AS table_kind
            FROM pg_namespace n
            LEFT JOIN pg_class c 
                ON c.relnamespace = n.oid 
                AND c.relkind IN ('r', 'v', 'm', 'f')
            WHERE n.nspname NOT IN ('pg_toast', 'pg_catalog', 'information_schema')
            ORDER BY n.nspname, c.relname
            "#,
            &[],
        )
        .await?;

    // Group by schema using BTreeMap for consistent ordering
    let mut schemas_map: BTreeMap<String, Vec<TableStructure>> = BTreeMap::new();

    for row in &rows {
        let schema_name: String = row.get(0);
        let table_name: Option<String> = row.get(1);
        let table_kind: Option<i8> = row.get(2);

        let tables = schemas_map.entry(schema_name).or_default();

        // Only add if table exists (LEFT JOIN may produce nulls for empty schemas)
        if let (Some(name), Some(kind)) = (table_name, table_kind) {
            let table_type = match kind as u8 as char {
                'r' => TableType::Table,
                'v' => TableType::View,
                'm' => TableType::MaterializedView,
                'f' => TableType::Foreign,
                _ => TableType::Table,
            };

            tables.push(TableStructure { name, table_type });
        }
    }

    let schemas = schemas_map
        .into_iter()
        .map(|(name, tables)| SchemaStructure { name, tables })
        .collect();

    Ok(DatabaseStructure {
        schemas,
        fetched_at: Utc::now().timestamp_millis(),
    })
}
