use std::collections::BTreeMap;

use mongodb::bson::{doc, Bson, Document};
use mongodb::{Client, Database};

use crate::error::Result;
use crate::models::{ColumnInfo, DatabaseInfo, IndexInfo, TableInfo, TableType};

use super::executor::collect_cursor;
use super::types::bson_type_name;

/// Number of documents sampled to infer a collection's "columns".
const COLUMN_SAMPLE_SIZE: i64 = 100;

/// Só os nomes (`nameOnly`): com o tamanho, o servidor abre as estatísticas de
/// cada database (288 ms para 300 bancos na baseline; 0,6 ms sem). Com
/// `authorizedDatabases`, um usuário sem `listDatabases` vê os bancos em que
/// tem permissão em vez de um erro.
pub async fn list_databases(client: &Client) -> Result<Vec<DatabaseInfo>> {
    let names = client.list_database_names().authorized_databases(true).await?;

    Ok(names
        .into_iter()
        .map(|name| DatabaseInfo {
            name,
            size_bytes: None,
        })
        .collect())
}

pub async fn list_tables(db: &Database, schema_name: &str) -> Result<Vec<TableInfo>> {
    let mut names = db.list_collection_names().await?;
    names.sort();

    Ok(names
        .into_iter()
        .map(|name| TableInfo {
            name,
            schema: schema_name.to_string(),
            table_type: TableType::Table,
            row_estimate: None,
            size_bytes: None,
        })
        .collect())
}

/// Infer columns by sampling documents: the union of top-level fields, with
/// the BSON type of the first non-null occurrence. A field missing from any
/// sampled document is reported as nullable.
pub async fn list_columns(db: &Database, collection: &str) -> Result<Vec<ColumnInfo>> {
    let coll = db.collection::<Document>(collection);
    let cursor = coll.find(Document::new()).limit(COLUMN_SAMPLE_SIZE).await?;
    let docs = collect_cursor(cursor, Some(COLUMN_SAMPLE_SIZE as usize)).await?;

    struct FieldStats {
        type_name: &'static str,
        present_in: usize,
        saw_null: bool,
    }

    // BTreeMap for stable alphabetical order; `_id` sorts first naturally
    // but we pin it explicitly below.
    let mut fields: BTreeMap<String, FieldStats> = BTreeMap::new();

    for doc in &docs {
        for (key, value) in doc {
            let entry = fields.entry(key.clone()).or_insert(FieldStats {
                type_name: "null",
                present_in: 0,
                saw_null: false,
            });
            entry.present_in += 1;
            if matches!(value, Bson::Null) {
                entry.saw_null = true;
            } else if entry.type_name == "null" {
                entry.type_name = bson_type_name(value);
            }
        }
    }

    let total = docs.len();
    let mut columns: Vec<ColumnInfo> = Vec::with_capacity(fields.len());

    // `_id` first
    if let Some(stats) = fields.remove("_id") {
        columns.push(field_to_column("_id", stats.type_name, false, true));
    }

    for (name, stats) in fields {
        let nullable = stats.saw_null || stats.present_in < total;
        columns.push(field_to_column(&name, stats.type_name, nullable, false));
    }

    for (i, col) in columns.iter_mut().enumerate() {
        col.ordinal = i as i32 + 1;
    }

    Ok(columns)
}

fn field_to_column(
    name: &str,
    data_type: &str,
    is_nullable: bool,
    is_primary_key: bool,
) -> ColumnInfo {
    ColumnInfo {
        name: name.to_string(),
        ordinal: 0, // assigned sequentially by the caller
        data_type: data_type.to_string(),
        is_nullable,
        default_value: None,
        is_primary_key,
        is_foreign_key: false,
    }
}

pub async fn list_indexes(db: &Database, collection: &str) -> Result<Vec<IndexInfo>> {
    let coll = db.collection::<Document>(collection);
    let mut cursor = coll.list_indexes().await?;

    let mut indexes = Vec::new();
    while cursor.advance().await? {
        let model = cursor.deserialize_current()?;

        let name = model
            .options
            .as_ref()
            .and_then(|o| o.name.clone())
            .unwrap_or_default();

        let columns: Vec<String> = model.keys.keys().map(String::from).collect();

        // Index kind from key values: 1/-1 → regular b-tree-like, otherwise
        // the special type ("text", "2dsphere", "hashed", ...)
        let index_type = model
            .keys
            .values()
            .find_map(|v| match v {
                Bson::String(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_else(|| "regular".to_string());

        let is_unique = model
            .options
            .as_ref()
            .and_then(|o| o.unique)
            .unwrap_or(false);
        let is_primary = name == "_id_";

        indexes.push(IndexInfo {
            name,
            columns,
            is_unique: is_unique || is_primary,
            is_primary,
            index_type,
        });
    }

    Ok(indexes)
}

/// Tamanho em disco da coleção (armazenamento + índices) via `$collStats` —
/// só da coleção aberta: em massa é um aggregate por coleção (49 s para 5.000
/// coleções em rede remota, na baseline). Em cluster shardeado vem um
/// documento por shard, então somamos. Views, coleções sem permissão de
/// `collStats` ou qualquer erro viram `None`.
pub(super) async fn collection_size(db: &Database, collection: &str) -> Option<i64> {
    let pipeline = vec![doc! { "$collStats": { "storageStats": {} } }];
    let cursor = db
        .collection::<Document>(collection)
        .aggregate(pipeline)
        .await
        .ok()?;
    let docs = collect_cursor(cursor, None).await.ok()?;

    let mut total = None;
    for stats in docs.iter().filter_map(|d| d.get_document("storageStats").ok()) {
        // `totalSize` só existe a partir do 4.4; antes somamos as partes
        let size = bson_number(stats.get("totalSize")).or_else(|| {
            let storage = bson_number(stats.get("storageSize"))?;
            Some(storage + bson_number(stats.get("totalIndexSize")).unwrap_or(0))
        })?;
        total = Some(total.unwrap_or(0) + size);
    }
    total
}

/// Os contadores do `collStats` chegam como Int32, Int64 ou Double conforme
/// o tamanho e a versão do servidor.
fn bson_number(value: Option<&Bson>) -> Option<i64> {
    match value? {
        Bson::Int32(n) => Some(*n as i64),
        Bson::Int64(n) => Some(*n),
        Bson::Double(n) => Some(*n as i64),
        _ => None,
    }
}
