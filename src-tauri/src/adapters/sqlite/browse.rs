use std::time::Instant;

use rusqlite::Connection;
use sqlparser::dialect::SQLiteDialect;
use sqlparser::parser::Parser;
use sqlparser::tokenizer::Token;

use crate::error::{Error, Result};
use crate::models::{
    EditableInfo, QueryColumnInfo, QueryResult, SortDirection, SortSpec, TableDataRequest,
};

use super::util::{
    db_err, pk_columns, quote_ident, storage_class_name, table_columns, value_to_string,
};

pub fn fetch_table_data(
    conn: &Connection,
    request: TableDataRequest,
    database: &str,
) -> Result<QueryResult> {
    // Valida que a tabela existe, dá os nomes para recusar sort desconhecido e
    // a PK, que decide a ordenação padrão e a editabilidade.
    let columns_meta = table_columns(conn, &request.table)?;
    let primary_key_columns = pk_columns(&columns_meta);
    let column_names: Vec<String> = columns_meta.iter().map(|c| c.name.clone()).collect();

    let clauses = build_clauses(&request, &column_names, &primary_key_columns)?;

    let base = format!(
        "FROM {}{}",
        quote_ident(&request.table),
        clauses.where_clause,
    );

    // Diferente do Postgres, não há cast `::text` aqui: a conversão para string
    // acontece no Rust (`value_to_string`), então WHERE e ORDER BY continuam
    // operando sobre os tipos reais das colunas.
    let select_list = column_names
        .iter()
        .map(|name| quote_ident(name))
        .collect::<Vec<_>>()
        .join(", ");

    // +1 linha para descobrir o has_more sem uma segunda query; na exportação,
    // `LIMIT -1` é o jeito do SQLite de pedir tudo sem abrir mão do OFFSET.
    // limit/offset são i64 do TableDataRequest, então interpolar não abre brecha.
    let limit = if request.unlimited {
        -1
    } else {
        request.limit + 1
    };
    let select = format!(
        "SELECT {select_list} {base}{} LIMIT {} OFFSET {}",
        clauses.order_clause, limit, request.offset,
    );

    let start = Instant::now();
    let mut stmt = conn.prepare(&select).map_err(db_err)?;

    let mut columns: Vec<QueryColumnInfo> = columns_meta
        .iter()
        .map(|column| QueryColumnInfo {
            name: column.name.clone(),
            type_name: column.declared_type.clone(),
            type_oid: None,
        })
        .collect();

    let mut result_rows: Vec<Vec<Option<String>>> = Vec::new();
    let mut rows = stmt.query([]).map_err(db_err)?;
    let mut first_row = true;

    while let Some(row) = rows.next().map_err(db_err)? {
        let mut cells = Vec::with_capacity(columns.len());

        for (index, column) in columns.iter_mut().enumerate() {
            let value = row.get_ref(index).map_err(db_err)?;

            // Coluna declarada sem tipo (o SQLite permite): usa a classe de
            // armazenamento da primeira linha para o front escolher o editor.
            if first_row && column.type_name.is_empty() {
                column.type_name = storage_class_name(value).to_string();
            }

            cells.push(value_to_string(value));
        }

        result_rows.push(cells);
        first_row = false;
    }

    let execution_time_ms = start.elapsed().as_millis() as u64;

    let has_more = !request.unlimited && result_rows.len() as i64 > request.limit;
    if has_more {
        result_rows.truncate(request.limit as usize);
    }

    // Roda depois da página: o COUNT é do rodapé e não deve entrar no tempo
    // de execução exibido.
    let total_count = if request.count_total {
        Some(
            conn.query_row(&format!("SELECT COUNT(*) {base}"), [], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(db_err)?,
        )
    } else {
        None
    };

    let editable_info = detect_editable_info(
        database,
        &request.table,
        &primary_key_columns,
        &columns,
    );

    Ok(QueryResult {
        row_count: result_rows.len(),
        columns,
        rows: result_rows,
        total_count,
        has_more,
        execution_time_ms,
        editable_info,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Montagem do SQL (pura, testável)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug)]
struct BuiltClauses {
    /// `" WHERE (...)"` com espaço na frente, ou vazio
    where_clause: String,
    /// `" ORDER BY ..."` com espaço na frente, ou vazio
    order_clause: String,
}

fn build_clauses(
    request: &TableDataRequest,
    columns: &[String],
    primary_key_columns: &[String],
) -> Result<BuiltClauses> {
    let where_clause = match normalize_where_expr(request.where_expr.as_deref())? {
        Some(expr) => format!(" WHERE ({expr})"),
        None => String::new(),
    };

    Ok(BuiltClauses {
        where_clause,
        order_clause: build_order_clause(&request.sort, columns, primary_key_columns)?,
    })
}

/// Interpreta `raw` como uma única expressão SQL. Vazio/em branco → `None`.
/// Um `WHERE` no começo é removido para o texto poder casar com o rótulo da UI.
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

    let dialect = SQLiteDialect {};
    let mut parser = Parser::new(&dialect)
        .try_with_sql(expr)
        .map_err(|e| Error::InvalidQuery(format!("Invalid WHERE expression: {e}")))?;
    parser
        .parse_expr()
        .map_err(|e| Error::InvalidQuery(format!("Invalid WHERE expression: {e}")))?;

    // Sobrou token depois da expressão → alguém tentou emendar outro comando
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
    columns: &[String],
    primary_key_columns: &[String],
) -> Result<String> {
    // Ordenação padrão: PK desc, para a paginação por OFFSET ser determinística.
    // Sem PK, fica sem ORDER BY.
    if sort.is_empty() {
        if primary_key_columns.is_empty() {
            return Ok(String::new());
        }

        let parts: Vec<String> = primary_key_columns
            .iter()
            .map(|column| format!("{} DESC", quote_ident(column)))
            .collect();

        return Ok(format!(" ORDER BY {}", parts.join(", ")));
    }

    let parts: Vec<String> = sort
        .iter()
        .map(|spec| {
            if !columns.contains(&spec.column) {
                return Err(Error::InvalidQuery(format!(
                    "Unknown column: {}",
                    spec.column
                )));
            }

            let direction = match spec.direction {
                SortDirection::Asc => "ASC",
                SortDirection::Desc => "DESC",
            };

            Ok(format!("{} {}", quote_ident(&spec.column), direction))
        })
        .collect::<Result<_>>()?;

    Ok(format!(" ORDER BY {}", parts.join(", ")))
}

fn detect_editable_info(
    database: &str,
    table: &str,
    primary_key_columns: &[String],
    result_columns: &[QueryColumnInfo],
) -> Option<EditableInfo> {
    if primary_key_columns.is_empty() {
        return None;
    }

    let primary_key_column_indices: Vec<usize> = primary_key_columns
        .iter()
        .filter_map(|pk| result_columns.iter().position(|c| c.name == *pk))
        .collect();

    if primary_key_column_indices.len() != primary_key_columns.len() {
        return None;
    }

    Some(EditableInfo {
        // Sem nível de schema: o "schema" é o próprio banco, como no Mongo
        schema: database.to_string(),
        table: table.to_string(),
        primary_key_columns: primary_key_columns.to_vec(),
        primary_key_column_indices,
    })
}

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
            unlimited: false,
        }
    }

    fn columns() -> Vec<String> {
        vec!["id".to_string(), "name".to_string()]
    }

    fn pk() -> Vec<String> {
        vec!["id".to_string()]
    }

    #[test]
    fn no_sort_defaults_to_pk_desc() {
        let built = build_clauses(&req(None, vec![]), &columns(), &pk()).unwrap();
        assert_eq!(built.where_clause, "");
        assert_eq!(built.order_clause, " ORDER BY \"id\" DESC");
    }

    #[test]
    fn no_sort_no_pk_has_no_order() {
        let built = build_clauses(&req(None, vec![]), &columns(), &[]).unwrap();
        assert_eq!(built.order_clause, "");
    }

    #[test]
    fn empty_where_is_omitted() {
        let built = build_clauses(&req(Some("   "), vec![]), &columns(), &pk()).unwrap();
        assert_eq!(built.where_clause, "");
    }

    #[test]
    fn where_expr_is_parenthesized() {
        let built = build_clauses(&req(Some("id = 1"), vec![]), &columns(), &pk()).unwrap();
        assert_eq!(built.where_clause, " WHERE (id = 1)");
    }

    #[test]
    fn leading_where_keyword_is_stripped() {
        let built = build_clauses(
            &req(Some("WHERE id = 1 AND name LIKE '%ana%'"), vec![]),
            &columns(),
            &pk(),
        )
        .unwrap();
        assert_eq!(built.where_clause, " WHERE (id = 1 AND name LIKE '%ana%')");
    }

    #[test]
    fn union_after_expr_is_rejected() {
        let err = build_clauses(
            &req(Some("id = 1 UNION SELECT 1"), vec![]),
            &columns(),
            &pk(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("single WHERE expression"));
    }

    #[test]
    fn invalid_expr_is_rejected() {
        let err = build_clauses(&req(Some("id ="), vec![]), &columns(), &pk()).unwrap_err();
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
            &columns(),
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
            &columns(),
            &pk(),
        )
        .unwrap_err();
        assert!(err.to_string().contains("Unknown column"));
    }
}
