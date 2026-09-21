use chrono::Utc;
use rusqlite::Connection;

use crate::error::Result;
use crate::models::{
    ColumnInfo, DatabaseInfo, DatabaseStructure, IndexInfo, SchemaStructure, TableInfo,
    TableStructure, TableType,
};

use super::util::{db_err, table_columns, value_to_string};

/// Bancos visíveis nesta conexão: o `main`, o `temp` e os que tiverem sido
/// anexados com `ATTACH`. O tamanho vem do arquivo em disco — `:memory:` e o
/// `temp` não têm arquivo e ficam sem tamanho.
pub fn list_databases(conn: &Connection) -> Result<Vec<DatabaseInfo>> {
    let mut stmt = conn
        .prepare("SELECT name, file FROM pragma_database_list ORDER BY seq")
        .map_err(db_err)?;

    let databases = stmt
        .query_map([], |row| {
            let name: String = row.get(0)?;
            let file: String = row.get(1)?;

            let size_bytes = (!file.is_empty())
                .then(|| std::fs::metadata(&file).ok())
                .flatten()
                .map(|meta| meta.len() as i64);

            Ok(DatabaseInfo { name, size_bytes })
        })
        .map_err(db_err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(db_err)?;

    Ok(databases)
}

/// Tabelas e views do banco. As internas (`sqlite_sequence`, `sqlite_stat1`…)
/// ficam de fora, como o Postgres esconde os schemas de catálogo.
///
/// `row_estimate` fica em `None` de propósito: o SQLite não guarda estatística
/// de linhas, e um `COUNT(*)` por tabela deixaria a árvore cara de abrir.
pub fn list_tables(conn: &Connection, schema_name: &str) -> Result<Vec<TableInfo>> {
    let mut stmt = conn
        .prepare(
            "SELECT name, type FROM sqlite_schema \
             WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%' \
             ORDER BY name",
        )
        .map_err(db_err)?;

    let tables = stmt
        .query_map([], |row| {
            let name: String = row.get(0)?;
            let kind: String = row.get(1)?;

            Ok(TableInfo {
                name,
                schema: schema_name.to_string(),
                table_type: match kind.as_str() {
                    "view" => TableType::View,
                    _ => TableType::Table,
                },
                row_estimate: None,
            })
        })
        .map_err(db_err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(db_err)?;

    Ok(tables)
}

pub fn list_columns(conn: &Connection, table: &str) -> Result<Vec<ColumnInfo>> {
    let columns = table_columns(conn, table)?;
    let foreign_keys = foreign_key_columns(conn, table)?;

    Ok(columns
        .into_iter()
        .enumerate()
        .map(|(index, column)| ColumnInfo {
            ordinal: index as i32 + 1,
            is_primary_key: column.pk_position > 0,
            is_foreign_key: foreign_keys.contains(&column.name),
            data_type: column.declared_type,
            is_nullable: !column.not_null,
            default_value: column.default_value,
            name: column.name,
        })
        .collect())
}

fn foreign_key_columns(conn: &Connection, table: &str) -> Result<Vec<String>> {
    let mut stmt = conn
        .prepare("SELECT \"from\" FROM pragma_foreign_key_list(?1)")
        .map_err(db_err)?;

    let columns = stmt
        .query_map([table], |row| row.get(0))
        .map_err(db_err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(db_err)?;

    Ok(columns)
}

pub fn list_indexes(conn: &Connection, table: &str) -> Result<Vec<IndexInfo>> {
    let mut list_stmt = conn
        .prepare("SELECT name, \"unique\", origin FROM pragma_index_list(?1) ORDER BY seq")
        .map_err(db_err)?;

    // (nome, é_único, origem) — a origem 'pk' marca o índice da chave primária
    let indexes = list_stmt
        .query_map([table], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i32>(1)? != 0,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(db_err)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(db_err)?;

    let mut info_stmt = conn
        .prepare("SELECT name FROM pragma_index_info(?1) ORDER BY seqno")
        .map_err(db_err)?;

    indexes
        .into_iter()
        .map(|(name, is_unique, origin)| {
            let columns = info_stmt
                .query_map([&name], |row| {
                    // Índice sobre expressão não tem nome de coluna
                    Ok(value_to_string(row.get_ref(0)?)
                        .unwrap_or_else(|| "(expr)".to_string()))
                })
                .map_err(db_err)?
                .collect::<rusqlite::Result<Vec<_>>>()
                .map_err(db_err)?;

            Ok(IndexInfo {
                name,
                columns,
                is_unique,
                is_primary: origin == "pk",
                // O SQLite só tem índice b-tree
                index_type: "btree".to_string(),
            })
        })
        .collect()
}

/// O SQLite não tem nível de schema, então a estrutura vem com um único
/// "schema" com o nome do banco — mesma convenção do Mongo e do Redis.
pub fn list_schemas_with_tables(conn: &Connection, schema_name: &str) -> Result<DatabaseStructure> {
    let tables = list_tables(conn, schema_name)?
        .into_iter()
        .map(|table| TableStructure {
            name: table.name,
            table_type: table.table_type,
        })
        .collect();

    Ok(DatabaseStructure {
        schemas: vec![SchemaStructure {
            name: schema_name.to_string(),
            tables,
        }],
        fetched_at: Utc::now().timestamp_millis(),
    })
}
