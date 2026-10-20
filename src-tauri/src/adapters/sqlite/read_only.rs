//! Execução só-leitura no SQLite (`ReadOnlyOp::Sql`), para o assistente.
//!
//! Segura sozinha, mesmo que o guard deixe passar uma escrita:
//! - **`PRAGMA query_only = ON`** durante a execução (o SQLite recusa qualquer
//!   mudança no arquivo), com o valor anterior restaurado no fim — a conexão é
//!   a mesma do editor;
//! - **um statement só:** o `prepare` do rusqlite compila o primeiro e ignora
//!   o resto em silêncio (sem a feature `extra_check`), então o texto passa
//!   pelo `Batch`, que compila um a um sem executar, e mais de um é recusado;
//! - **dentro de uma subquery**, com quebras de linha para um `--` no fim não
//!   comentar o resto;
//! - **tempo com teto:** o SQLite não tem `statement_timeout`, então um vigia
//!   interrompe a consulta (`sqlite3_interrupt`) quando o tempo acaba.

use std::sync::mpsc;
use std::time::Instant;

use rusqlite::{Batch, Connection, ErrorCode, Statement};

use crate::adapters::{ReadOnlyLimits, STATEMENT_TIMEOUT_MESSAGE};
use crate::error::{Error, Result};
use crate::models::{QueryColumnInfo, QueryOptions, QueryResult};

use super::util::{db_err, storage_class_name, value_to_string};

pub fn execute(
    conn: &Connection,
    sql: &str,
    page: &QueryOptions,
    limits: &ReadOnlyLimits,
) -> Result<QueryResult> {
    let sql = crate::adapters::trim_statement(sql);
    if sql.is_empty() {
        return Err(Error::InvalidQuery("Empty query".into()));
    }

    let (limit, offset) = limits.page(page.limit, page.offset);
    // limit/offset são números já limitados: interpolar não abre brecha
    let paged = format!("SELECT * FROM (\n{sql}\n) LIMIT {} OFFSET {offset}", limit + 1);

    query_only(conn, |conn| with_deadline(conn, limits, |conn| read_page(conn, &paged, limit)))
}

/// Roda `work` com `PRAGMA query_only` ligado e devolve o valor anterior no
/// fim, dê certo ou não.
fn query_only<T>(conn: &Connection, work: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
    let previous: i64 = conn
        .query_row("PRAGMA query_only", [], |row| row.get(0))
        .map_err(db_err)?;
    conn.execute_batch("PRAGMA query_only = ON").map_err(db_err)?;

    let result = work(conn);

    let restore = conn
        .execute_batch(&format!("PRAGMA query_only = {}", if previous != 0 { "ON" } else { "OFF" }))
        .map_err(db_err);
    let value = result?;
    restore?;
    Ok(value)
}

/// Interrompe a consulta se ela passar do tempo. O vigia é uma thread que
/// espera o fim da consulta ou o prazo, o que vier antes.
fn with_deadline<T>(
    conn: &Connection,
    limits: &ReadOnlyLimits,
    work: impl FnOnce(&Connection) -> Result<T>,
) -> Result<T> {
    let interrupt = conn.get_interrupt_handle();
    let timeout = limits.statement_timeout;
    let (done, finished) = mpsc::channel::<()>();
    let watchdog = std::thread::spawn(move || {
        if finished.recv_timeout(timeout) == Err(mpsc::RecvTimeoutError::Timeout) {
            // Sem consulta rodando (acabou bem no prazo) não tem efeito
            interrupt.interrupt();
        }
    });

    let result = work(conn);
    let _ = done.send(());
    let _ = watchdog.join();

    result.map_err(|error| match error {
        Error::Query(message) if message.contains("interrupted") => {
            Error::Query(STATEMENT_TIMEOUT_MESSAGE.into())
        }
        other => other,
    })
}

fn read_page(conn: &Connection, paged: &str, limit: i64) -> Result<QueryResult> {
    let started = Instant::now();
    let mut stmt = prepare_single(conn, paged)?;

    let mut columns: Vec<QueryColumnInfo> = stmt
        .columns()
        .iter()
        .map(|column| QueryColumnInfo {
            name: column.name().to_string(),
            type_name: column.decl_type().unwrap_or_default().to_string(),
            type_oid: None,
        })
        .collect();

    let mut rows: Vec<Vec<Option<String>>> = Vec::new();
    let mut cursor = stmt.query([]).map_err(interrupted_or_db_err)?;
    while let Some(row) = cursor.next().map_err(interrupted_or_db_err)? {
        let mut cells = Vec::with_capacity(columns.len());
        for (index, column) in columns.iter_mut().enumerate() {
            let value = row.get_ref(index).map_err(db_err)?;
            // Expressão não tem tipo declarado: vale a classe da primeira linha
            if rows.is_empty() && column.type_name.is_empty() {
                column.type_name = storage_class_name(value).to_string();
            }
            cells.push(value_to_string(value));
        }
        rows.push(cells);
        if rows.len() as i64 > limit {
            break;
        }
    }
    let execution_time_ms = started.elapsed().as_millis() as u64;

    let has_more = rows.len() as i64 > limit;
    rows.truncate(limit as usize);

    Ok(QueryResult {
        row_count: rows.len(),
        columns,
        rows,
        total_count: None,
        has_more,
        execution_time_ms,
        // O resultado do assistente nunca é editável (RN-16)
        editable_info: None,
    })
}

/// Compila `sql` exigindo um statement só: `SELECT 1); DELETE …; --` vira dois
/// dentro da subquery, e o segundo nunca pode ficar escondido no texto.
fn prepare_single<'conn>(conn: &'conn Connection, sql: &str) -> Result<Statement<'conn>> {
    let mut batch = Batch::new(conn, sql);
    let stmt = batch
        .next()
        .map_err(interrupted_or_db_err)?
        .ok_or_else(|| Error::InvalidQuery("Empty query".into()))?;
    if batch.next().map_err(interrupted_or_db_err)?.is_some() {
        return Err(Error::InvalidQuery("only one statement is allowed".into()));
    }
    Ok(stmt)
}

/// O `sqlite3_interrupt` chega como `OperationInterrupted`; o resto segue o
/// `db_err` do adapter.
fn interrupted_or_db_err(error: rusqlite::Error) -> Error {
    match &error {
        rusqlite::Error::SqliteFailure(failure, _)
            if failure.code == ErrorCode::OperationInterrupted =>
        {
            Error::Query("interrupted".into())
        }
        _ => db_err(error),
    }
}

/// Aqui o guard fica **desligado**: o executor é chamado direto, e a escrita
/// tem que ser barrada pelo próprio SQLite. Sem servidor: arquivo temporário.
#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn fixture(name: &str) -> (Connection, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!("octapus_db_sqlite_read_only_{name}.db"));
        let _ = std::fs::remove_file(&path);
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, score REAL);
             WITH RECURSIVE g(n) AS (SELECT 1 UNION ALL SELECT n + 1 FROM g WHERE n < 25)
             INSERT INTO users (name, score) SELECT 'user ' || n, n * 1.5 FROM g;",
        )
        .unwrap();
        (conn, path)
    }

    fn read(conn: &Connection, sql: &str) -> Result<QueryResult> {
        execute(conn, sql, &QueryOptions::default(), &ReadOnlyLimits::default())
    }

    fn user_count(conn: &Connection) -> i64 {
        conn.query_row("SELECT count(*) FROM users", [], |row| row.get(0)).unwrap()
    }

    fn query_only_flag(conn: &Connection) -> i64 {
        conn.query_row("PRAGMA query_only", [], |row| row.get(0)).unwrap()
    }

    #[test]
    fn reads_and_pages() {
        let (conn, path) = fixture("reads");

        let page = QueryOptions { limit: 10, offset: 20, ..QueryOptions::default() };
        let result = execute(
            &conn,
            "SELECT id, name, score FROM users ORDER BY id;",
            &page,
            &ReadOnlyLimits::default(),
        )
        .unwrap();
        assert_eq!(result.row_count, 5);
        assert!(!result.has_more);
        assert_eq!(result.rows[0], vec![
            Some("21".into()),
            Some("user 21".into()),
            Some("31.5".into()),
        ]);
        assert_eq!(result.columns[1].type_name, "TEXT");
        assert!(result.editable_info.is_none());

        let first = execute(
            &conn,
            "SELECT * FROM users",
            &QueryOptions { limit: 10, ..QueryOptions::default() },
            &ReadOnlyLimits::default(),
        )
        .unwrap();
        assert_eq!(first.row_count, 10);
        assert!(first.has_more);

        let result = read(&conn, "SELECT 1 AS a -- fim").unwrap();
        assert_eq!(result.rows, vec![vec![Some("1".into())]]);
        assert_eq!(query_only_flag(&conn), 0, "query_only volta ao que era");

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn writes_never_reach_the_file() {
        let (conn, path) = fixture("writes");
        let before = user_count(&conn);

        for sql in [
            "SELECT 1; DELETE FROM users",
            "SELECT 1); DELETE FROM users; --",
            "DELETE FROM users",
            "DELETE FROM users RETURNING id",
            "WITH gone AS (DELETE FROM users RETURNING id) SELECT * FROM gone",
            "INSERT INTO users (name) VALUES ('x') RETURNING id",
            "DROP TABLE users",
            "PRAGMA query_only = OFF",
            "ATTACH DATABASE ':memory:' AS evil",
        ] {
            assert!(read(&conn, sql).is_err(), "devia falhar: {sql}");
            assert_eq!(query_only_flag(&conn), 0, "query_only restaurado depois de: {sql}");
        }
        assert_eq!(user_count(&conn), before);

        // O texto escondido depois do primeiro statement é recusado, não ignorado
        let error = read(&conn, "SELECT 1); DELETE FROM users; --").unwrap_err();
        assert_eq!(error.to_string(), "Invalid query: only one statement is allowed");

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn query_only_blocks_writes_on_the_connection() {
        let (conn, path) = fixture("query_only");

        let result = query_only(&conn, |conn| {
            conn.execute("DELETE FROM users", []).map_err(db_err)
        });
        let error = result.unwrap_err().to_string();
        assert!(error.contains("readonly"), "{error}");
        assert_eq!(user_count(&conn), 25);
        assert_eq!(query_only_flag(&conn), 0);

        drop(conn);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn long_query_is_interrupted() {
        let (conn, path) = fixture("timeout");
        let limits = ReadOnlyLimits {
            statement_timeout: Duration::from_millis(150),
            ..ReadOnlyLimits::default()
        };

        let started = Instant::now();
        let error = execute(
            &conn,
            "WITH RECURSIVE c(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM c) SELECT max(x) FROM c",
            &QueryOptions::default(),
            &limits,
        )
        .unwrap_err();
        assert!(started.elapsed() < Duration::from_secs(3));
        assert_eq!(error.to_string(), format!("Query error: {STATEMENT_TIMEOUT_MESSAGE}"));

        // A conexão continua usável e sem query_only
        assert_eq!(read(&conn, "SELECT 1").unwrap().row_count, 1);
        assert_eq!(query_only_flag(&conn), 0);

        drop(conn);
        let _ = std::fs::remove_file(path);
    }
}
