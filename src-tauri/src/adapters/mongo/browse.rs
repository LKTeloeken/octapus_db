use std::time::Instant;

use mongodb::bson::{doc, Bson, Document};
use mongodb::Database;

use crate::error::{Error, Result};
use crate::models::{
    ColumnFilter, FilterOp, QueryResult, SortDirection, TableDataRequest,
};

use super::executor::{collect_cursor, documents_to_table, editable_for, scalar_variants};
use super::types::{like_to_regex, parse_scalar};

pub async fn fetch_table_data(
    db: &Database,
    database_name: &str,
    request: TableDataRequest,
) -> Result<QueryResult> {
    let coll = db.collection::<Document>(&request.table);

    let filter = build_filter(&request.filters)?;
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
// Filter/sort translation (pure, unit-testable)
// ─────────────────────────────────────────────────────────────────────────────

fn build_filter(filters: &[ColumnFilter]) -> Result<Document> {
    let conditions: Vec<Document> = filters
        .iter()
        .map(build_condition)
        .collect::<Result<_>>()?;

    Ok(match conditions.len() {
        0 => Document::new(),
        1 => conditions.into_iter().next().unwrap(),
        // $and allows multiple conditions on the same field
        _ => doc! { "$and": conditions },
    })
}

fn build_condition(filter: &ColumnFilter) -> Result<Document> {
    let field = filter.column.as_str();

    let first_value = || {
        filter.values.first().map(String::as_str).ok_or_else(|| {
            Error::InvalidQuery(format!("Filter on field '{field}' requires a value"))
        })
    };

    let condition = match filter.op {
        // Use all plausible typed interpretations so "42" matches both the
        // number 42 and the string "42"
        FilterOp::Eq => doc! { field: { "$in": scalar_variants(first_value()?) } },
        FilterOp::Ne => doc! { field: { "$nin": scalar_variants(first_value()?) } },
        FilterOp::In => {
            if filter.values.is_empty() {
                return Err(Error::InvalidQuery(format!(
                    "Filter 'in' on field '{field}' requires at least one value"
                )));
            }
            let variants: Vec<Bson> = filter
                .values
                .iter()
                .flat_map(|v| scalar_variants(v))
                .collect();
            doc! { field: { "$in": variants } }
        }
        FilterOp::Like => doc! { field: { "$regex": like_to_regex(first_value()?) } },
        FilterOp::Gt => doc! { field: { "$gt": parse_scalar(first_value()?) } },
        FilterOp::Gte => doc! { field: { "$gte": parse_scalar(first_value()?) } },
        FilterOp::Lt => doc! { field: { "$lt": parse_scalar(first_value()?) } },
        FilterOp::Lte => doc! { field: { "$lte": parse_scalar(first_value()?) } },
        // {field: null} matches both null and missing fields
        FilterOp::IsNull => doc! { field: Bson::Null },
        FilterOp::NotNull => doc! { field: { "$ne": Bson::Null } },
    };

    Ok(condition)
}

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

    fn filter(column: &str, op: FilterOp, values: &[&str]) -> ColumnFilter {
        ColumnFilter {
            column: column.into(),
            op,
            values: values.iter().map(|v| v.to_string()).collect(),
        }
    }

    #[test]
    fn eq_matches_typed_and_string() {
        let doc = build_filter(&[filter("age", FilterOp::Eq, &["42"])]).unwrap();
        assert_eq!(
            doc,
            doc! { "age": { "$in": [Bson::Int64(42), Bson::String("42".into())] } }
        );
    }

    #[test]
    fn eq_plain_string_single_variant() {
        let doc = build_filter(&[filter("name", FilterOp::Eq, &["ana"])]).unwrap();
        assert_eq!(doc, doc! { "name": { "$in": [Bson::String("ana".into())] } });
    }

    #[test]
    fn multiple_filters_use_and() {
        let doc = build_filter(&[
            filter("age", FilterOp::Gte, &["18"]),
            filter("age", FilterOp::Lt, &["60"]),
        ])
        .unwrap();
        assert_eq!(
            doc,
            doc! { "$and": [
                { "age": { "$gte": Bson::Int64(18) } },
                { "age": { "$lt": Bson::Int64(60) } },
            ]}
        );
    }

    #[test]
    fn like_becomes_anchored_regex() {
        let doc = build_filter(&[filter("name", FilterOp::Like, &["%ana%"])]).unwrap();
        assert_eq!(doc, doc! { "name": { "$regex": "^.*ana.*$" } });
    }

    #[test]
    fn null_filters() {
        assert_eq!(
            build_filter(&[filter("x", FilterOp::IsNull, &[])]).unwrap(),
            doc! { "x": Bson::Null }
        );
        assert_eq!(
            build_filter(&[filter("x", FilterOp::NotNull, &[])]).unwrap(),
            doc! { "x": { "$ne": Bson::Null } }
        );
    }

    #[test]
    fn empty_in_rejected() {
        assert!(build_filter(&[filter("x", FilterOp::In, &[])]).is_err());
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

        // Browse: filter age >= 5, sort idx desc, paginate
        let result = adapter
            .fetch_table_data(TableDataRequest {
                schema: None,
                table: "users".into(),
                filters: vec![filter("age", FilterOp::Gte, &["5"])],
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

        // ages cycle 1..9,0; age >= 5 → i % 10 in 5..=9 → 15 of 30 docs
        assert_eq!(result.total_count, Some(15));
        assert_eq!(result.row_count, 10);
        assert!(result.has_more);
        let editable = result.editable_info.expect("editable by _id");
        assert_eq!(editable.primary_key_columns, vec!["_id".to_string()]);
        // First row: highest idx with age>=5 → idx=29 (29%10=9)
        let idx_col = result.columns.iter().position(|c| c.name == "idx").unwrap();
        assert_eq!(result.rows[0][idx_col].as_deref(), Some("29"));

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
