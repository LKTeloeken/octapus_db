use std::collections::HashMap;
use std::time::Instant;

use rusqlite::types::Value;
use rusqlite::{params_from_iter, Connection};
use sqlparser::ast::{GroupByExpr, SetExpr, Statement as SqlStatement, TableFactor};
use sqlparser::dialect::SQLiteDialect;
use sqlparser::parser::Parser;

use crate::error::{Error, Result};
use crate::models::{
    EditableInfo, QueryColumnInfo, QueryOptions, QueryResult, RowEdit, RowInsert, StatementResult,
};

use super::util::{
    coerce_value, db_err, pk_columns, quote_ident, storage_class_name, table_columns,
    value_to_string,
};

pub fn execute_query(
    conn: &Connection,
    query: &str,
    options: QueryOptions,
    database: &str,
) -> Result<QueryResult> {
    let trimmed = query.trim().trim_end_matches(';').trim();

    if trimmed.is_empty() {
        return Err(Error::InvalidQuery("Empty query".into()));
    }

    // Só o que tem corpo de SELECT pode entrar numa subquery para paginar; um
    // PRAGMA devolve linhas mas quebraria dentro do `SELECT * FROM (...)`.
    let wrappable = is_wrappable(trimmed);

    let (exec_query, limit) = if wrappable && !options.unlimited {
        // limit/offset são i64 do próprio QueryOptions, então interpolar não
        // abre brecha — e um parâmetro ligado colidiria com um `?` que o
        // usuário tenha escrito na query.
        (
            format!(
                "SELECT * FROM ({trimmed}) LIMIT {} OFFSET {}",
                options.limit + 1, // +1 para descobrir o has_more sem outra query
                options.offset
            ),
            Some(options.limit),
        )
    } else {
        (trimmed.to_string(), None)
    };

    let start = Instant::now();
    let mut stmt = conn.prepare(&exec_query).map_err(db_err)?;

    // Statement sem colunas não devolve linhas (INSERT/UPDATE/DDL no editor
    // livre). Nesse caso o texto original roda inteiro por `execute_statement`,
    // que aceita script com vários comandos — o `prepare` pararia no primeiro.
    if stmt.column_count() == 0 {
        drop(stmt);
        let statement = execute_statement(conn, trimmed)?;

        return Ok(QueryResult {
            columns: vec![],
            rows: vec![],
            row_count: 0,
            total_count: None,
            has_more: false,
            execution_time_ms: statement.execution_time_ms,
            editable_info: None,
        });
    }

    let mut columns: Vec<QueryColumnInfo> = stmt
        .columns()
        .iter()
        .map(|column| QueryColumnInfo {
            name: column.name().to_string(),
            // Tipo declarado no CREATE TABLE; expressão não tem, e aí o nome
            // sai da classe de armazenamento da primeira linha (logo abaixo).
            type_name: column.decl_type().unwrap_or_default().to_string(),
            // Sem equivalente ao OID do Postgres
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

            if first_row && column.type_name.is_empty() {
                column.type_name = storage_class_name(value).to_string();
            }

            cells.push(value_to_string(value));
        }

        result_rows.push(cells);
        first_row = false;
    }

    let execution_time_ms = start.elapsed().as_millis() as u64;

    let has_more = matches!(limit, Some(l) if result_rows.len() as i64 > l);
    if let Some(l) = limit {
        result_rows.truncate(l as usize);
    }

    // Sem total a query continua valendo: o COUNT é só a contagem do rodapé.
    let total_count = if options.count_total && wrappable {
        conn.query_row(
            &format!("SELECT COUNT(*) FROM ({trimmed})"),
            [],
            |row| row.get::<_, i64>(0),
        )
        .ok()
    } else {
        None
    };

    let editable_info = if wrappable {
        detect_editable_info(conn, trimmed, database, &columns)
    } else {
        None
    };

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

pub fn execute_statement(conn: &Connection, statement: &str) -> Result<StatementResult> {
    let start = Instant::now();

    // `execute_batch` aceita DDL e script com vários comandos; o `execute`
    // pararia no primeiro e recusaria statement que devolve linhas.
    conn.execute_batch(statement).map_err(db_err)?;

    Ok(StatementResult {
        // `changes()` conta só o último comando: num script de vários INSERTs o
        // rodapé mostra o do fim, não a soma. O SQLite expõe o acumulado em
        // `sqlite3_total_changes`, que o rusqlite 0.31 ainda não embrulha.
        affected_rows: conn.changes(),
        execution_time_ms: start.elapsed().as_millis() as u64,
    })
}

pub fn apply_row_edits(
    conn: &mut Connection,
    editable: &EditableInfo,
    edits: Vec<RowEdit>,
) -> Result<StatementResult> {
    if edits.is_empty() {
        return Ok(no_op());
    }

    let types = column_types(conn, &editable.table)?;

    let start = Instant::now();
    let tx = conn.transaction().map_err(db_err)?;
    let mut total_affected: u64 = 0;

    for edit in &edits {
        if edit.changes.is_empty() {
            continue;
        }

        if edit.pk_values.len() != editable.primary_key_columns.len() {
            return Err(Error::InvalidQuery(
                "Primary key value count does not match primary key columns".into(),
            ));
        }

        let mut params: Vec<Value> = Vec::new();

        // ── SET ──────────────────────────────────────────────────────────
        let mut set_clauses: Vec<String> = Vec::new();
        for (column, value) in &edit.changes {
            params.push(coerce_value(value.as_deref(), declared_type(&types, column)?)?);
            set_clauses.push(format!("{} = ?{}", quote_ident(column), params.len()));
        }

        // ── WHERE (identifica a linha pela PK) ───────────────────────────
        let mut where_clauses: Vec<String> = Vec::new();
        for (column, value) in editable.primary_key_columns.iter().zip(&edit.pk_values) {
            params.push(coerce_value(value.as_deref(), declared_type(&types, column)?)?);
            where_clauses.push(format!("{} = ?{}", quote_ident(column), params.len()));
        }

        let sql = format!(
            "UPDATE {} SET {} WHERE {}",
            quote_ident(&editable.table),
            set_clauses.join(", "),
            where_clauses.join(" AND "),
        );

        let affected = tx
            .execute(&sql, params_from_iter(params))
            .map_err(|e| Error::Query(format!("Failed to update row: {e}")))?;

        total_affected += affected as u64;
    }

    tx.commit().map_err(db_err)?;

    Ok(StatementResult {
        affected_rows: total_affected,
        execution_time_ms: start.elapsed().as_millis() as u64,
    })
}

pub fn insert_rows(
    conn: &mut Connection,
    editable: &EditableInfo,
    rows: Vec<RowInsert>,
) -> Result<StatementResult> {
    if rows.is_empty() {
        return Ok(no_op());
    }

    let types = column_types(conn, &editable.table)?;

    let start = Instant::now();
    let tx = conn.transaction().map_err(db_err)?;
    let mut total_affected: u64 = 0;

    for row in &rows {
        // Só as colunas preenchidas são enviadas; as de fora ficam com o
        // default da tabela (AUTOINCREMENT, DEFAULT, rowid).
        if row.values.is_empty() {
            continue;
        }

        let mut params: Vec<Value> = Vec::new();
        let mut columns: Vec<String> = Vec::new();
        let mut placeholders: Vec<String> = Vec::new();

        for (column, value) in &row.values {
            params.push(coerce_value(value.as_deref(), declared_type(&types, column)?)?);
            columns.push(quote_ident(column));
            placeholders.push(format!("?{}", params.len()));
        }

        let sql = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            quote_ident(&editable.table),
            columns.join(", "),
            placeholders.join(", "),
        );

        let affected = tx
            .execute(&sql, params_from_iter(params))
            .map_err(|e| Error::Query(format!("Failed to insert row: {e}")))?;

        total_affected += affected as u64;
    }

    tx.commit().map_err(db_err)?;

    Ok(StatementResult {
        affected_rows: total_affected,
        execution_time_ms: start.elapsed().as_millis() as u64,
    })
}

pub fn delete_rows(
    conn: &mut Connection,
    editable: &EditableInfo,
    pk_values: Vec<Vec<Option<String>>>,
) -> Result<StatementResult> {
    if pk_values.is_empty() {
        return Ok(no_op());
    }

    let types = column_types(conn, &editable.table)?;

    let start = Instant::now();
    let tx = conn.transaction().map_err(db_err)?;
    let mut total_affected: u64 = 0;

    for pk in &pk_values {
        if pk.len() != editable.primary_key_columns.len() {
            return Err(Error::InvalidQuery(
                "Primary key value count does not match primary key columns".into(),
            ));
        }

        let mut params: Vec<Value> = Vec::new();
        let mut where_clauses: Vec<String> = Vec::new();

        for (column, value) in editable.primary_key_columns.iter().zip(pk) {
            params.push(coerce_value(value.as_deref(), declared_type(&types, column)?)?);
            where_clauses.push(format!("{} = ?{}", quote_ident(column), params.len()));
        }

        let sql = format!(
            "DELETE FROM {} WHERE {}",
            quote_ident(&editable.table),
            where_clauses.join(" AND "),
        );

        let affected = tx
            .execute(&sql, params_from_iter(params))
            .map_err(|e| Error::Query(format!("Failed to delete row: {e}")))?;

        total_affected += affected as u64;
    }

    tx.commit().map_err(db_err)?;

    Ok(StatementResult {
        affected_rows: total_affected,
        execution_time_ms: start.elapsed().as_millis() as u64,
    })
}

pub fn execute_transaction(
    conn: &mut Connection,
    statements: Vec<String>,
) -> Result<Vec<StatementResult>> {
    let tx = conn.transaction().map_err(db_err)?;
    let mut results = Vec::with_capacity(statements.len());

    for statement in &statements {
        let start = Instant::now();
        tx.execute_batch(statement).map_err(db_err)?;

        results.push(StatementResult {
            // Mesma ressalva do `execute_statement` sobre scripts
            affected_rows: tx.changes(),
            execution_time_ms: start.elapsed().as_millis() as u64,
        });
    }

    tx.commit().map_err(db_err)?;

    Ok(results)
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn no_op() -> StatementResult {
    StatementResult {
        affected_rows: 0,
        execution_time_ms: 0,
    }
}

/// Tipo declarado de cada coluna, usado pela coerção por afinidade.
fn column_types(conn: &Connection, table: &str) -> Result<HashMap<String, String>> {
    Ok(table_columns(conn, table)?
        .into_iter()
        .map(|column| (column.name, column.declared_type))
        .collect())
}

fn declared_type<'a>(types: &'a HashMap<String, String>, column: &str) -> Result<&'a str> {
    types
        .get(column)
        .map(String::as_str)
        .ok_or_else(|| Error::InvalidQuery(format!("Unknown column: {column}")))
}

fn is_wrappable(query: &str) -> bool {
    let first_word = query.split_whitespace().next().unwrap_or("");
    matches!(first_word.to_uppercase().as_str(), "SELECT" | "WITH" | "VALUES")
}

/// Um resultado só é editável quando dá para saber de qual linha física cada
/// linha da grade veio: uma única tabela, sem join nem agregação, e com a PK
/// inteira presente no resultado.
///
/// O Postgres resolve isso pelo `table_oid` que vem no protocolo; o SQLite não
/// expõe essa metainformação pelo rusqlite, então a origem sai do parser.
fn detect_editable_info(
    conn: &Connection,
    query: &str,
    database: &str,
    result_columns: &[QueryColumnInfo],
) -> Option<EditableInfo> {
    let table = single_source_table(query)?;

    let primary_key_columns = pk_columns(&table_columns(conn, &table).ok()?);
    if primary_key_columns.is_empty() {
        return None;
    }

    let primary_key_column_indices: Vec<usize> = primary_key_columns
        .iter()
        .filter_map(|pk| result_columns.iter().position(|c| c.name == *pk))
        .collect();

    // Sem a PK completa no resultado não dá para mirar a linha no UPDATE
    if primary_key_column_indices.len() != primary_key_columns.len() {
        return None;
    }

    Some(EditableInfo {
        schema: database.to_string(),
        table,
        primary_key_columns,
        primary_key_column_indices,
    })
}

fn single_source_table(sql: &str) -> Option<String> {
    let mut statements = Parser::parse_sql(&SQLiteDialect {}, sql).ok()?;

    if statements.len() != 1 {
        return None;
    }

    let SqlStatement::Query(query) = statements.remove(0) else {
        return None;
    };

    if query.with.is_some() {
        return None;
    }

    // UNION/EXCEPT e subquery no corpo não têm uma origem única
    let SetExpr::Select(select) = *query.body else {
        return None;
    };

    let ungrouped = matches!(
        &select.group_by,
        GroupByExpr::Expressions(expressions, modifiers)
            if expressions.is_empty() && modifiers.is_empty()
    );

    if select.distinct.is_some() || select.having.is_some() || !ungrouped {
        return None;
    }

    let [from] = select.from.as_slice() else {
        return None;
    };

    if !from.joins.is_empty() {
        return None;
    }

    let TableFactor::Table {
        name, args: None, ..
    } = &from.relation
    else {
        return None;
    };

    // `banco.tabela` só existe depois de um ATTACH; o nome da tabela é o último
    Some(name.0.last()?.as_ident()?.value.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_select_shaped_queries_are_paginated() {
        assert!(is_wrappable("SELECT * FROM users"));
        assert!(is_wrappable("  with x as (select 1) select * from x"));
        assert!(is_wrappable("VALUES (1), (2)"));
        // PRAGMA devolve linhas, mas não sobrevive dentro de uma subquery
        assert!(!is_wrappable("PRAGMA table_info(users)"));
        assert!(!is_wrappable("INSERT INTO users (name) VALUES ('ana')"));
    }

    #[test]
    fn single_table_select_is_editable() {
        assert_eq!(
            single_source_table("SELECT id, name FROM users WHERE id > 10"),
            Some("users".to_string())
        );
        assert_eq!(
            single_source_table("select * from main.users order by id desc limit 5"),
            Some("users".to_string())
        );
    }

    #[test]
    fn queries_without_a_single_source_row_are_not_editable() {
        // Join: a linha da grade não corresponde a uma linha física só
        assert_eq!(
            single_source_table("SELECT * FROM users u JOIN orders o ON o.user_id = u.id"),
            None
        );
        assert_eq!(single_source_table("SELECT COUNT(*) FROM users GROUP BY age"), None);
        assert_eq!(single_source_table("SELECT DISTINCT name FROM users"), None);
        assert_eq!(
            single_source_table("SELECT * FROM users UNION SELECT * FROM admins"),
            None
        );
        assert_eq!(single_source_table("WITH x AS (SELECT 1) SELECT * FROM x"), None);
        assert_eq!(single_source_table("SELECT 1"), None);
        assert_eq!(single_source_table("DELETE FROM users"), None);
    }
}
