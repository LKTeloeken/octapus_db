use std::collections::HashMap;
use std::time::Instant;

use deadpool_postgres::Pool;

use crate::error::{Error, Result};
use crate::models::{
    EditableInfo, QueryColumnInfo, QueryResult, SortDirection, SortSpec, TableDataRequest,
};
use sqlparser::dialect::PostgreSqlDialect;
use sqlparser::parser::Parser;
use sqlparser::tokenizer::Token;

use super::executor::extract_text_rows_typed;
use super::util::{get_columns_ordered, quote_ident};

const DEFAULT_SCHEMA: &str = "public";

pub async fn fetch_table_data(pool: &Pool, request: TableDataRequest) -> Result<QueryResult> {
    let client = pool.get().await?;

    let schema = request.schema.as_deref().unwrap_or(DEFAULT_SCHEMA);

    // Fetch column metadata and PK columns in parallel (independent round-trips).
    // Ordered column metadata also validates that the table exists and gives us
    // the column names for rejecting unknown sort columns. PK columns drive the
    // default ordering (stable pagination) and editability.
    let (columns_meta, pk_columns) = tokio::join!(
        get_columns_ordered(&client, schema, &request.table),
        fetch_pk_columns(&client, schema, &request.table),
    );
    let columns_meta = columns_meta?;
    let type_map: HashMap<String, String> = columns_meta
        .iter()
        .map(|c| (c.name.clone(), c.format_type.clone()))
        .collect();

    let clauses = build_clauses(&request, &type_map, &pk_columns)?;

    let base = format!(
        "FROM {}.{}{}",
        quote_ident(schema),
        quote_ident(&request.table),
        clauses.where_clause,
    );

    // Cast every column to text in SQL so Postgres does all type→text rendering
    // (handles jsonb/numeric/uuid/arrays/enums) and Rust only reads strings.
    // The cast happens in an OUTER layer: the inner query keeps the real typed
    // columns so WHERE/ORDER BY operate on them (e.g. numeric sort, not text).
    let select_list = columns_meta
        .iter()
        .map(|c| {
            let ident = quote_ident(&c.name);
            format!("{ident}::text AS {ident}")
        })
        .collect::<Vec<_>>()
        .join(", ");

    // +1 row to detect has_more without a second query
    let inner = format!(
        "SELECT * {base}{} LIMIT {} OFFSET {}",
        clauses.order_clause,
        request.limit + 1,
        request.offset,
    );
    let select = format!("SELECT {select_list} FROM ({inner}) AS __q");

    // Run the data query and the optional COUNT(*) pipelined on the same
    // connection so the count scan overlaps the fetch instead of running after
    // it. Only the data query is timed, so `executionTimeMs` reflects the page
    // fetch — not the (often heavier) count scan.
    let exec_started = Instant::now();
    let data_fut = async {
        let rows = client.query(&select, &[]).await;
        (rows, exec_started.elapsed())
    };
    let count_fut = async {
        if request.count_total {
            let count_sql = format!("SELECT COUNT(*) {base}");
            Ok::<Option<i64>, Error>(Some(
                client.query_one(&count_sql, &[]).await?.get::<_, i64>(0),
            ))
        } else {
            Ok(None)
        }
    };
    let ((rows, exec_elapsed), total_count) = tokio::join!(data_fut, count_fut);
    let rows = rows?;
    let total_count = total_count?;
    let execution_time_ms = exec_elapsed.as_millis() as u64;

    let has_more = rows.len() as i64 > request.limit;
    let rows_to_process = if has_more {
        &rows[..request.limit as usize]
    } else {
        &rows[..]
    };

    let columns: Vec<QueryColumnInfo> = columns_meta
        .iter()
        .map(|c| QueryColumnInfo {
            name: c.name.clone(),
            type_name: c.type_name.clone(),
            type_oid: Some(c.type_oid),
        })
        .collect();
    let result_rows = extract_text_rows_typed(rows_to_process);

    let editable_info =
        detect_editable_info(schema, &request.table, &pk_columns, &columns);

    Ok(QueryResult {
        row_count: rows_to_process.len(),
        columns,
        rows: result_rows,
        total_count,
        has_more,
        execution_time_ms,
        editable_info,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// SQL building (pure, unit-testable)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug)]
struct BuiltClauses {
    /// Leading-space `" WHERE (...)"` or empty
    where_clause: String,
    /// Leading-space `" ORDER BY ..."` or empty
    order_clause: String,
}

fn build_clauses(
    request: &TableDataRequest,
    type_map: &HashMap<String, String>,
    pk_columns: &[String],
) -> Result<BuiltClauses> {
    let where_clause = match normalize_where_expr(request.where_expr.as_deref())? {
        Some(expr) => format!(" WHERE ({expr})"),
        None => String::new(),
    };

    let order_clause = build_order_clause(&request.sort, type_map, pk_columns)?;

    Ok(BuiltClauses {
        where_clause,
        order_clause,
    })
}

/// Parse `raw` as a single SQL expression. Empty/whitespace → `None`.
/// A leading `WHERE` keyword is stripped so the input can match the UI label.
fn normalize_where_expr(raw: Option<&str>) -> Result<Option<String>> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let expr = strip_leading_where(trimmed);
    if expr.is_empty() {
        return Ok(None);
    }

    let dialect = PostgreSqlDialect {};
    let mut parser = Parser::new(&dialect)
        .try_with_sql(expr)
        .map_err(|e| Error::InvalidQuery(format!("Invalid WHERE expression: {e}")))?;
    parser
        .parse_expr()
        .map_err(|e| Error::InvalidQuery(format!("Invalid WHERE expression: {e}")))?;

    if !matches!(parser.peek_token().token, Token::EOF) {
        return Err(Error::InvalidQuery(
            "Filter must be a single WHERE expression".into(),
        ));
    }

    Ok(Some(expr.to_string()))
}

fn strip_leading_where(s: &str) -> &str {
    let trimmed = s.trim_start();
    let Some(rest) = trimmed.get(5..) else {
        return trimmed;
    };
    if trimmed[..5].eq_ignore_ascii_case("where")
        && (rest.is_empty() || rest.starts_with(char::is_whitespace))
    {
        return rest.trim_start();
    }
    trimmed
}

fn build_order_clause(
    sort: &[SortSpec],
    type_map: &HashMap<String, String>,
    pk_columns: &[String],
) -> Result<String> {
    // Default ordering: PK desc. Keeps OFFSET pagination deterministic; falls
    // back to no ordering when the table has no primary key.
    if sort.is_empty() {
        if pk_columns.is_empty() {
            return Ok(String::new());
        }
        let parts: Vec<String> = pk_columns
            .iter()
            .map(|col| format!("{} DESC", quote_ident(col)))
            .collect();
        return Ok(format!(" ORDER BY {}", parts.join(", ")));
    }

    let parts: Vec<String> = sort
        .iter()
        .map(|s| {
            // Validate the column exists (also guards quoting)
            column_type(&s.column, type_map)?;
            let dir = match s.direction {
                SortDirection::Asc => "ASC",
                SortDirection::Desc => "DESC",
            };
            Ok(format!("{} {}", quote_ident(&s.column), dir))
        })
        .collect::<Result<_>>()?;

    Ok(format!(" ORDER BY {}", parts.join(", ")))
}

fn column_type<'a>(
    column: &str,
    type_map: &'a HashMap<String, String>,
) -> Result<&'a str> {
    type_map
        .get(column)
        .map(String::as_str)
        .ok_or_else(|| Error::InvalidQuery(format!("Unknown column: {column}")))
}

// ─────────────────────────────────────────────────────────────────────────────
// Editable info (table is known, so no query introspection needed)
// ─────────────────────────────────────────────────────────────────────────────

/// Primary-key column names in index order, or empty if the table has no PK.
/// Used both for the default ordering and to detect editability.
async fn fetch_pk_columns(
    client: &deadpool_postgres::Object,
    schema: &str,
    table: &str,
) -> Vec<String> {
    let Ok(pk_rows) = client
        .query(
            "SELECT a.attname \
             FROM pg_index ix \
             JOIN pg_class t ON t.oid = ix.indrelid \
             JOIN pg_namespace n ON n.oid = t.relnamespace \
             JOIN pg_attribute a ON a.attrelid = t.oid AND a.attnum = ANY(ix.indkey) \
             WHERE n.nspname = $1 AND t.relname = $2 AND ix.indisprimary \
             ORDER BY array_position(ix.indkey, a.attnum)",
            &[&schema, &table],
        )
        .await
    else {
        return Vec::new();
    };

    pk_rows.iter().map(|r| r.get(0)).collect()
}

fn detect_editable_info(
    schema: &str,
    table: &str,
    pk_column_names: &[String],
    result_columns: &[crate::models::QueryColumnInfo],
) -> Option<EditableInfo> {
    if pk_column_names.is_empty() {
        return None;
    }

    let pk_indices: Vec<usize> = pk_column_names
        .iter()
        .filter_map(|pk_name| result_columns.iter().position(|c| c.name == *pk_name))
        .collect();

    if pk_indices.len() != pk_column_names.len() {
        return None;
    }

    Some(EditableInfo {
        schema: schema.to_string(),
        table: table.to_string(),
        primary_key_columns: pk_column_names.to_vec(),
        primary_key_column_indices: pk_indices,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn req(where_expr: Option<&str>, sort: Vec<SortSpec>) -> TableDataRequest {
        TableDataRequest {
            schema: None,
            table: "users".into(),
            where_expr: where_expr.map(str::to_string),
            sort,
            limit: 100,
            offset: 0,
            count_total: false,
        }
    }

    fn types() -> HashMap<String, String> {
        HashMap::from([
            ("id".to_string(), "integer".to_string()),
            ("name".to_string(), "text".to_string()),
        ])
    }

    /// Default PK used by the clause tests.
    fn pk() -> Vec<String> {
        vec!["id".to_string()]
    }

    #[test]
    fn no_sort_defaults_to_pk_desc() {
        let built = build_clauses(&req(None, vec![]), &types(), &pk()).unwrap();
        assert_eq!(built.where_clause, "");
        assert_eq!(built.order_clause, " ORDER BY \"id\" DESC");
    }

    #[test]
    fn no_sort_no_pk_has_no_order() {
        let built = build_clauses(&req(None, vec![]), &types(), &[]).unwrap();
        assert_eq!(built.order_clause, "");
    }

    #[test]
    fn empty_where_is_omitted() {
        let built = build_clauses(&req(Some("   "), vec![]), &types(), &pk()).unwrap();
        assert_eq!(built.where_clause, "");
    }

    #[test]
    fn where_expr_is_parenthesized() {
        let built = build_clauses(&req(Some("id = 1"), vec![]), &types(), &pk()).unwrap();
        assert_eq!(built.where_clause, " WHERE (id = 1)");
    }

    #[test]
    fn leading_where_keyword_is_stripped() {
        let built =
            build_clauses(&req(Some("WHERE id = 1 AND name ILIKE '%ana%'"), vec![]), &types(), &pk())
                .unwrap();
        assert_eq!(
            built.where_clause,
            " WHERE (id = 1 AND name ILIKE '%ana%')"
        );
    }

    #[test]
    fn union_after_expr_is_rejected() {
        let err = build_clauses(
            &req(Some("id = 1 UNION SELECT 1"), vec![]),
            &types(),
            &pk(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("single WHERE expression"));
    }

    #[test]
    fn invalid_expr_is_rejected() {
        let err = build_clauses(&req(Some("id ="), vec![]), &types(), &pk()).unwrap_err();
        assert!(err.to_string().contains("Invalid WHERE expression"));
    }

    #[test]
    fn sort_multiple_columns() {
        let built = build_clauses(
            &req(
                None,
                vec![
                    SortSpec {
                        column: "name".into(),
                        direction: SortDirection::Asc,
                    },
                    SortSpec {
                        column: "id".into(),
                        direction: SortDirection::Desc,
                    },
                ],
            ),
            &types(),
            &pk(),
        )
        .unwrap();
        assert_eq!(built.order_clause, " ORDER BY \"name\" ASC, \"id\" DESC");
    }

    #[test]
    fn unknown_sort_column_rejected() {
        let err = build_clauses(
            &req(
                None,
                vec![SortSpec {
                    column: "nope".into(),
                    direction: SortDirection::Asc,
                }],
            ),
            &types(),
            &pk(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("Unknown column"));
    }

    // ── End-to-end (requires a local Postgres; run with `cargo test -- --ignored`) ──

    #[tokio::test]
    #[ignore = "requires local postgres on localhost:5432 (postgres/1234)"]
    async fn e2e_fetch_table_data() {
        use crate::adapters::postgres::PostgresAdapter;
        use crate::adapters::DatabaseAdapter;
        use crate::models::{DatabaseType, Server};

        let server = Server {
            id: Some(1),
            name: "e2e".into(),
            db_type: DatabaseType::Postgres,
            host: "localhost".into(),
            port: 5432,
            username: "postgres".into(),
            password: "1234".into(),
            default_database: None,
            ssl_enabled: false,
            connection_uri: None,
            created_at: 0,
        };

        let admin = PostgresAdapter::new(&server, "postgres").unwrap();
        admin
            .execute_statement("DROP DATABASE IF EXISTS octapus_db_e2e WITH (FORCE)")
            .await
            .unwrap();
        admin
            .execute_statement("CREATE DATABASE octapus_db_e2e")
            .await
            .unwrap();

        let adapter = PostgresAdapter::new(&server, "octapus_db_e2e").unwrap();
        adapter
            .execute_statement(
                "CREATE TABLE users (id serial PRIMARY KEY, name text, age int)",
            )
            .await
            .unwrap();
        adapter
            .execute_statement(
                "INSERT INTO users (name, age) \
                 SELECT 'user' || i, i % 50 FROM generate_series(1, 120) i",
            )
            .await
            .unwrap();

        let result = adapter
            .fetch_table_data(TableDataRequest {
                schema: None,
                table: "users".into(),
                where_expr: Some("age >= 10".into()),
                sort: vec![SortSpec {
                    column: "id".into(),
                    direction: SortDirection::Desc,
                }],
                limit: 20,
                offset: 0,
                count_total: true,
            })
            .await
            .unwrap();

        assert_eq!(result.row_count, 20);
        assert!(result.has_more);
        // i in 1..=120 with i % 50 >= 10 → 40 + 40 + 11 rows
        assert_eq!(result.total_count, Some(91));
        // Sorted by id DESC → first row is id=120 (age 20, passes the filter)
        assert_eq!(result.rows[0][0].as_deref(), Some("120"));

        let editable = result.editable_info.expect("PK table should be editable");
        assert_eq!(editable.primary_key_columns, vec!["id".to_string()]);
        assert_eq!(editable.schema, "public");

        // Trailing statement after the expression must be rejected by the parser
        let err = adapter
            .fetch_table_data(TableDataRequest {
                schema: None,
                table: "users".into(),
                where_expr: Some("id = 1 UNION SELECT 1".into()),
                sort: vec![],
                limit: 10,
                offset: 0,
                count_total: false,
            })
            .await
            .unwrap_err();
        assert!(err.to_string().contains("single WHERE expression"));

        drop(adapter);
        admin
            .execute_statement("DROP DATABASE octapus_db_e2e WITH (FORCE)")
            .await
            .unwrap();
    }
}
