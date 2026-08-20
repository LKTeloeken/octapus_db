use std::time::Instant;

use mongodb::bson::{doc, Document};
use mongodb::Database;

use crate::error::{Error, Result};
use crate::models::{QueryResult, SortDirection, TableDataRequest};

use super::executor::{collect_cursor, documents_to_table, editable_for};

pub async fn fetch_table_data(
    db: &Database,
    database_name: &str,
    request: TableDataRequest,
) -> Result<QueryResult> {
    if request
        .where_expr
        .as_deref()
        .is_some_and(|s| !s.trim().is_empty())
    {
        return Err(Error::InvalidQuery(
            "Raw WHERE is only supported for PostgreSQL".into(),
        ));
    }

    let coll = db.collection::<Document>(&request.table);

    let filter = Document::new();
    let sort = build_sort(&request);

    let start = Instant::now();

    let mut find = coll
        .find(filter.clone())
        .skip(request.offset.max(0) as u64)
        .limit(request.limit + 1); // +1 to detect has_more
    if !sort.is_empty() {
        find = find.sort(sort);
    }

    let cursor = find.await?;
    let mut docs = collect_cursor(cursor, Some(request.limit as usize + 1)).await?;

    let has_more = docs.len() as i64 > request.limit;
    if has_more {
        docs.truncate(request.limit as usize);
    }

    let total_count = if request.count_total {
        Some(coll.count_documents(filter).await? as i64)
    } else {
        None
    };

    let execution_time_ms = start.elapsed().as_millis() as u64;

    let (columns, rows) = documents_to_table(&docs);
    let editable_info = editable_for(database_name, &request.table, &columns);

    Ok(QueryResult {
        row_count: rows.len(),
        columns,
        rows,
        total_count,
        has_more,
        execution_time_ms,
        editable_info,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Sort translation (pure, unit-testable)
// ─────────────────────────────────────────────────────────────────────────────

fn build_sort(request: &TableDataRequest) -> Document {
    // Default ordering: `_id` desc (the PK), keeping skip/limit pagination
    // deterministic when the caller doesn't ask for a specific order.
    if request.sort.is_empty() {
        return doc! { "_id": -1 };
    }

    let mut sort = Document::new();
    for spec in &request.sort {
        let dir = match spec.direction {
            SortDirection::Asc => 1,
            SortDirection::Desc => -1,
        };
        sort.insert(spec.column.clone(), dir);
    }
    sort
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_request(sort: Vec<crate::models::SortSpec>) -> TableDataRequest {
        TableDataRequest {
            schema: None,
            table: "users".into(),
            where_expr: None,
            sort,
            limit: 10,
            offset: 0,
            count_total: false,
        }
    }

    #[test]
    fn default_sort_is_id_desc() {
        assert_eq!(build_sort(&empty_request(vec![])), doc! { "_id": -1 });
    }

    #[test]
    fn explicit_sort_uses_requested_columns() {
        let sort = build_sort(&empty_request(vec![crate::models::SortSpec {
            column: "idx".into(),
            direction: SortDirection::Desc,
        }]));
        assert_eq!(sort, doc! { "idx": -1 });
    }

    // ── End-to-end (requires a local MongoDB; run with `cargo test -- --ignored`) ──

    #[tokio::test]
    #[ignore = "requires local mongodb on localhost:27017 (no auth)"]
    async fn e2e_mongo_adapter() {
        use crate::adapters::mongo::MongoAdapter;
        use crate::adapters::DatabaseAdapter;
        use crate::models::{DatabaseType, QueryOptions, Server, SortSpec};

        let server = Server {
            id: Some(1),
            name: "e2e".into(),
            db_type: DatabaseType::Mongodb,
            host: "localhost".into(),
            port: 27017,
            username: String::new(),
            password: String::new(),
            default_database: None,
            ssl_enabled: false,
            connection_uri: None,
            created_at: 0,
        };

        let adapter = MongoAdapter::new(&server, "octapus_e2e").await.unwrap();
        adapter.test_connection().await.unwrap();

        // Seed via the shell-style editor path
        adapter
            .execute_statement("db.users.deleteMany({})")
            .await
            .unwrap();
        for i in 1..=30 {
            adapter
                .execute_statement(&format!(
                    "db.users.insertOne({{ idx: {i}, name: 'user{i}', age: {} }})",
                    i % 10
                ))
                .await
                .unwrap();
        }

        // Structure: collections + sampled columns
        let tables = adapter.list_tables("").await.unwrap();
        assert!(tables.iter().any(|t| t.name == "users"));
        let columns = adapter.list_columns("", "users").await.unwrap();
        assert_eq!(columns[0].name, "_id");
        assert!(columns[0].is_primary_key);
        assert!(columns
            .iter()
            .any(|c| c.name == "age" && (c.data_type == "int" || c.data_type == "long")));

        // Browse: sort idx desc, paginate
        let result = adapter
            .fetch_table_data(TableDataRequest {
                schema: None,
                table: "users".into(),
                where_expr: None,
                sort: vec![SortSpec {
                    column: "idx".into(),
                    direction: crate::models::SortDirection::Desc,
                }],
                limit: 10,
                offset: 0,
                count_total: true,
            })
            .await
            .unwrap();

        assert_eq!(result.total_count, Some(30));
        assert_eq!(result.row_count, 10);
        assert!(result.has_more);
        let editable = result.editable_info.expect("editable by _id");
        assert_eq!(editable.primary_key_columns, vec!["_id".to_string()]);
        let idx_col = result.columns.iter().position(|c| c.name == "idx").unwrap();
        assert_eq!(result.rows[0][idx_col].as_deref(), Some("30"));

        // Free-form query through the editor
        let q = adapter
            .execute_query(
                "db.users.find({ age: { $gte: 5 } })",
                QueryOptions { limit: 5, ..Default::default() },
            )
            .await
            .unwrap();
        assert_eq!(q.row_count, 5);
        assert!(q.has_more);

        let count = adapter
            .execute_query("db.users.countDocuments({})", QueryOptions::default())
            .await
            .unwrap();
        assert_eq!(count.rows[0][0].as_deref(), Some("30"));

        // Row edit by _id
        let id_value = result.rows[0][0].clone().unwrap();
        let edited = adapter
            .apply_row_edits(
                &editable,
                vec![crate::models::RowEdit {
                    pk_values: vec![Some(id_value)],
                    changes: vec![("name".to_string(), Some("renamed".to_string()))],
                }],
            )
            .await
            .unwrap();
        assert_eq!(edited.affected_rows, 1);

        // Cleanup
        adapter.execute_statement("db.users.drop()").await.unwrap();
    }
}
