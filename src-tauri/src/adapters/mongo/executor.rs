use std::time::Instant;

use mongodb::bson::{doc, Bson, Document};
use mongodb::{Cursor, Database};

use crate::error::{Error, Result};
use crate::models::{
    EditableInfo, QueryColumnInfo, QueryOptions, QueryResult, RowEdit, RowInsert, StatementResult,
};

use super::command::{parse_command, MongoCommand};
use super::types::{bson_to_string, bson_type_name, parse_scalar};

pub async fn execute_query(
    db: &Database,
    database_name: &str,
    query: &str,
    options: QueryOptions,
) -> Result<QueryResult> {
    let command = parse_command(query)?;
    let start = Instant::now();

    let result = match command {
        MongoCommand::Find {
            collection,
            filter,
            projection,
        } => {
            let coll = db.collection::<Document>(&collection);

            let mut find = coll.find(filter.clone());
            if let Some(p) = projection {
                find = find.projection(p);
            }
            if !options.unlimited {
                find = find
                    .skip(options.offset.max(0) as u64)
                    .limit(options.limit + 1); // +1 to detect has_more
            }

            let cursor = find.await?;
            let cap = (!options.unlimited).then_some(options.limit as usize + 1);
            let mut docs = collect_cursor(cursor, cap).await?;

            let has_more = !options.unlimited && docs.len() as i64 > options.limit;
            if has_more {
                docs.truncate(options.limit as usize);
            }

            let total_count = if options.count_total {
                Some(coll.count_documents(filter).await? as i64)
            } else {
                None
            };

            let (columns, rows) = documents_to_table(&docs);
            let editable_info = editable_for(database_name, &collection, &columns);

            QueryResult {
                row_count: rows.len(),
                columns,
                rows,
                total_count,
                has_more,
                execution_time_ms: 0,
                editable_info,
            }
        }

        MongoCommand::FindOne {
            collection,
            filter,
            projection,
        } => {
            let coll = db.collection::<Document>(&collection);
            let mut find = coll.find_one(filter);
            if let Some(p) = projection {
                find = find.projection(p);
            }
            let docs: Vec<Document> = find.await?.into_iter().collect();
            let (columns, rows) = documents_to_table(&docs);
            let editable_info = editable_for(database_name, &collection, &columns);

            QueryResult {
                row_count: rows.len(),
                columns,
                rows,
                total_count: None,
                has_more: false,
                execution_time_ms: 0,
                editable_info,
            }
        }

        MongoCommand::Aggregate {
            collection,
            pipeline,
        } => {
            let coll = db.collection::<Document>(&collection);
            let cursor = coll.aggregate(pipeline).await?;
            // Cap aggregation output like a paginated query unless unlimited
            let cap = (!options.unlimited).then_some(options.limit as usize + 1);
            let mut docs = collect_cursor(cursor, cap).await?;

            let has_more = !options.unlimited && docs.len() as i64 > options.limit;
            if has_more {
                docs.truncate(options.limit as usize);
            }

            let (columns, rows) = documents_to_table(&docs);
            QueryResult {
                row_count: rows.len(),
                columns,
                rows,
                total_count: None,
                has_more,
                execution_time_ms: 0,
                editable_info: None,
            }
        }

        MongoCommand::CountDocuments { collection, filter } => {
            let count = db
                .collection::<Document>(&collection)
                .count_documents(filter)
                .await?;
            single_value_result("count", "long", Some(count.to_string()))
        }

        MongoCommand::Distinct {
            collection,
            field,
            filter,
        } => {
            let values = db
                .collection::<Document>(&collection)
                .distinct(&field, filter)
                .await?;
            let type_name = values
                .first()
                .map(bson_type_name)
                .unwrap_or("mixed")
                .to_string();
            let rows: Vec<Vec<Option<String>>> =
                values.iter().map(|v| vec![bson_to_string(v)]).collect();

            QueryResult {
                columns: vec![QueryColumnInfo {
                    name: field,
                    type_name,
                    type_oid: None,
                }],
                row_count: rows.len(),
                rows,
                total_count: None,
                has_more: false,
                execution_time_ms: 0,
                editable_info: None,
            }
        }

        // Write commands executed from the editor: return a summary table
        write_command => {
            let summary = run_write(db, write_command).await?;
            let columns = summary
                .keys()
                .map(|k| QueryColumnInfo {
                    name: k.clone(),
                    type_name: "long".into(),
                    type_oid: None,
                })
                .collect();
            let row: Vec<Option<String>> =
                summary.values().map(bson_to_string).collect();

            QueryResult {
                columns,
                rows: vec![row],
                row_count: 1,
                total_count: None,
                has_more: false,
                execution_time_ms: 0,
                editable_info: None,
            }
        }
    };

    Ok(QueryResult {
        execution_time_ms: start.elapsed().as_millis() as u64,
        ..result
    })
}

pub async fn execute_statement(db: &Database, statement: &str) -> Result<StatementResult> {
    let command = parse_command(statement)?;

    if !command.is_write() {
        return Err(Error::InvalidQuery(
            "Read commands must be executed as queries".into(),
        ));
    }

    let start = Instant::now();
    let summary = run_write(db, command).await?;

    let affected = summary
        .get("modifiedCount")
        .or_else(|| summary.get("deletedCount"))
        .or_else(|| summary.get("insertedCount"))
        .and_then(Bson::as_i64)
        .unwrap_or(0);

    Ok(StatementResult {
        affected_rows: affected as u64,
        execution_time_ms: start.elapsed().as_millis() as u64,
    })
}

pub async fn apply_row_edits(
    db: &Database,
    editable: &EditableInfo,
    edits: Vec<RowEdit>,
) -> Result<StatementResult> {
    let coll = db.collection::<Document>(&editable.table);
    let start = Instant::now();
    let mut total_modified: u64 = 0;

    for edit in &edits {
        if edit.changes.is_empty() {
            continue;
        }

        let id_value = edit
            .pk_values
            .first()
            .and_then(|v| v.as_deref())
            .ok_or_else(|| Error::InvalidQuery("Missing _id value for edit".into()))?;

        let mut set_doc = Document::new();
        for (field, value) in &edit.changes {
            set_doc.insert(
                field.clone(),
                value.as_deref().map(parse_scalar).unwrap_or(Bson::Null),
            );
        }

        let result = coll
            .update_one(
                doc! { "_id": { "$in": scalar_variants(id_value) } },
                doc! { "$set": set_doc },
            )
            .await?;

        total_modified += result.modified_count;
    }

    Ok(StatementResult {
        affected_rows: total_modified,
        execution_time_ms: start.elapsed().as_millis() as u64,
    })
}

pub async fn insert_rows(
    db: &Database,
    editable: &EditableInfo,
    rows: Vec<RowInsert>,
) -> Result<StatementResult> {
    let coll = db.collection::<Document>(&editable.table);
    let start = Instant::now();
    let mut total_inserted: u64 = 0;

    for row in &rows {
        // Only filled fields are written; omitting `_id` lets MongoDB
        // generate an ObjectId automatically.
        if row.values.is_empty() {
            continue;
        }

        let mut document = Document::new();
        for (field, value) in &row.values {
            document.insert(
                field.clone(),
                value.as_deref().map(parse_scalar).unwrap_or(Bson::Null),
            );
        }

        coll.insert_one(document).await?;
        total_inserted += 1;
    }

    Ok(StatementResult {
        affected_rows: total_inserted,
        execution_time_ms: start.elapsed().as_millis() as u64,
    })
}

pub async fn delete_rows(
    db: &Database,
    editable: &EditableInfo,
    pk_values: Vec<Vec<Option<String>>>,
) -> Result<StatementResult> {
    let coll = db.collection::<Document>(&editable.table);
    let start = Instant::now();
    let mut total_deleted: u64 = 0;

    for pk in &pk_values {
        let id_value = pk
            .first()
            .and_then(|v| v.as_deref())
            .ok_or_else(|| Error::InvalidQuery("Missing _id value for delete".into()))?;

        let result = coll
            .delete_one(doc! { "_id": { "$in": scalar_variants(id_value) } })
            .await?;

        total_deleted += result.deleted_count;
    }

    Ok(StatementResult {
        affected_rows: total_deleted,
        execution_time_ms: start.elapsed().as_millis() as u64,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Shared helpers (also used by browse.rs)
// ─────────────────────────────────────────────────────────────────────────────

/// Flatten documents into a tabular result: columns are the union of top-level
/// keys (`_id` first, then first-seen order), values are display strings.
pub fn documents_to_table(
    docs: &[Document],
) -> (Vec<QueryColumnInfo>, Vec<Vec<Option<String>>>) {
    let mut column_names: Vec<String> = Vec::new();
    let mut column_types: Vec<&'static str> = Vec::new();

    for doc in docs {
        for (key, value) in doc {
            match column_names.iter().position(|c| c == key) {
                Some(idx) => {
                    if column_types[idx] == "null" && !matches!(value, Bson::Null) {
                        column_types[idx] = bson_type_name(value);
                    }
                }
                None => {
                    if key == "_id" {
                        column_names.insert(0, key.clone());
                        column_types.insert(0, bson_type_name(value));
                    } else {
                        column_names.push(key.clone());
                        column_types.push(bson_type_name(value));
                    }
                }
            }
        }
    }

    let rows: Vec<Vec<Option<String>>> = docs
        .iter()
        .map(|doc| {
            column_names
                .iter()
                .map(|col| doc.get(col).and_then(bson_to_string))
                .collect()
        })
        .collect();

    let columns = column_names
        .into_iter()
        .zip(column_types)
        .map(|(name, type_name)| QueryColumnInfo {
            name,
            type_name: type_name.to_string(),
            type_oid: None,
        })
        .collect();

    (columns, rows)
}

/// Documents are editable by `_id` when it is present in the result.
pub fn editable_for(
    database: &str,
    collection: &str,
    columns: &[QueryColumnInfo],
) -> Option<EditableInfo> {
    let id_index = columns.iter().position(|c| c.name == "_id")?;
    Some(EditableInfo {
        schema: database.to_string(),
        table: collection.to_string(),
        primary_key_columns: vec!["_id".to_string()],
        primary_key_column_indices: vec![id_index],
    })
}

/// All plausible BSON interpretations of a frontend string value, so filters
/// match regardless of how the value is stored (e.g. `42` vs `"42"`).
pub fn scalar_variants(raw: &str) -> Vec<Bson> {
    let parsed = parse_scalar(raw);
    if matches!(parsed, Bson::String(_)) {
        vec![parsed]
    } else {
        vec![parsed, Bson::String(raw.to_string())]
    }
}

pub async fn collect_cursor(
    mut cursor: Cursor<Document>,
    cap: Option<usize>,
) -> Result<Vec<Document>> {
    let mut docs = Vec::new();
    while cursor.advance().await? {
        docs.push(cursor.deserialize_current()?);
        if let Some(max) = cap {
            if docs.len() >= max {
                break;
            }
        }
    }
    Ok(docs)
}

fn single_value_result(name: &str, type_name: &str, value: Option<String>) -> QueryResult {
    QueryResult {
        columns: vec![QueryColumnInfo {
            name: name.to_string(),
            type_name: type_name.to_string(),
            type_oid: None,
        }],
        rows: vec![vec![value]],
        row_count: 1,
        total_count: None,
        has_more: false,
        execution_time_ms: 0,
        editable_info: None,
    }
}

/// Execute a write command, returning a shell-like summary document.
async fn run_write(db: &Database, command: MongoCommand) -> Result<Document> {
    let summary = match command {
        MongoCommand::InsertOne {
            collection,
            document,
        } => {
            let result = db
                .collection::<Document>(&collection)
                .insert_one(document)
                .await?;
            doc! { "insertedCount": 1_i64, "insertedId": result.inserted_id }
        }
        MongoCommand::InsertMany {
            collection,
            documents,
        } => {
            let result = db
                .collection::<Document>(&collection)
                .insert_many(documents)
                .await?;
            doc! { "insertedCount": result.inserted_ids.len() as i64 }
        }
        MongoCommand::UpdateOne {
            collection,
            filter,
            update,
        } => {
            let result = db
                .collection::<Document>(&collection)
                .update_one(filter, update)
                .await?;
            doc! {
                "matchedCount": result.matched_count as i64,
                "modifiedCount": result.modified_count as i64,
            }
        }
        MongoCommand::UpdateMany {
            collection,
            filter,
            update,
        } => {
            let result = db
                .collection::<Document>(&collection)
                .update_many(filter, update)
                .await?;
            doc! {
                "matchedCount": result.matched_count as i64,
                "modifiedCount": result.modified_count as i64,
            }
        }
        MongoCommand::DeleteOne { collection, filter } => {
            let result = db
                .collection::<Document>(&collection)
                .delete_one(filter)
                .await?;
            doc! { "deletedCount": result.deleted_count as i64 }
        }
        MongoCommand::DeleteMany { collection, filter } => {
            let result = db
                .collection::<Document>(&collection)
                .delete_many(filter)
                .await?;
            doc! { "deletedCount": result.deleted_count as i64 }
        }
        MongoCommand::Drop { collection } => {
            db.collection::<Document>(&collection).drop().await?;
            doc! { "dropped": Bson::String(collection) }
        }
        read => {
            return Err(Error::InvalidQuery(format!(
                "Not a write command: {read:?}"
            )))
        }
    };

    Ok(summary)
}
