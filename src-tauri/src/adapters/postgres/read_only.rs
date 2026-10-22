//! Execução só-leitura no Postgres (`ReadOnlyOp::Sql`), para o assistente.
//!
//! Segura sozinha, mesmo que o guard deixe passar uma escrita:
//! - **um statement só:** a consulta vai pelo protocolo estendido
//!   (`prepare`/`query`), que recusa `SELECT 1; DROP …` — o `simple_query` do
//!   editor livre aceitaria, e um `COMMIT` no meio sairia da transação;
//! - **`BEGIN READ ONLY`**, com `statement_timeout` e `lock_timeout` locais, e
//!   **`ROLLBACK` sempre**, antes de a conexão voltar ao pool;
//! - **dentro de uma subquery** (`SELECT … FROM (\n…\n) AS __q`), que só aceita
//!   consulta: `INSERT … RETURNING` e CTE que modifica dados nem parseiam ali.
//!   As quebras de linha impedem que um `--` no fim comente o resto.
//!
//! O preflight (`explain`) usa as mesmas barreiras e roda `EXPLAIN (FORMAT
//! JSON)` sem `ANALYZE`: o planejador acha coluna ou tabela inexistente sem
//! executar nada. Os erros do servidor saem como `Error::DbQuery`, com o
//! SQLSTATE para o assistente classificar (`assistant/failure.rs`).

use std::time::Instant;

use deadpool_postgres::Pool;
use tokio_postgres::types::{FromSql, Type};
use tokio_postgres::Transaction;

use crate::adapters::ReadOnlyLimits;
use crate::error::{Error, Result};
use crate::models::{QueryOptions, QueryResult, ReadOnlyPlan};

use super::executor::{columns_from_statement, extract_text_rows_typed, PidGuard};
use super::util::quote_ident;
use super::QueryRegistry;

pub async fn execute(
    pool: &Pool,
    sql: &str,
    page: &QueryOptions,
    limits: &ReadOnlyLimits,
    registry: &QueryRegistry,
) -> Result<QueryResult> {
    let sql = crate::adapters::trim_statement(sql);
    if sql.is_empty() {
        return Err(Error::InvalidQuery("Empty query".into()));
    }

    let mut client = pool.get().await?;

    // Com `queryId`, o `cancel_query` do app alcança esta execução (botão
    // cancelar do assistente)
    let _pid_guard = match page.query_id.as_deref() {
        Some(query_id) => {
            let pid: i32 = client.query_one("SELECT pg_backend_pid()", &[]).await?.get(0);
            registry.lock().unwrap().insert(query_id.to_string(), pid);
            Some(PidGuard::new(registry, query_id))
        }
        None => None,
    };

    let tx = client.build_transaction().read_only(true).start().await.map_err(db_error)?;
    let result = run(&tx, sql, page, limits).await;
    finish(tx, result).await
}

/// O plano da consulta, sem executá-la: `EXPLAIN (FORMAT JSON)` dentro das
/// mesmas barreiras da execução.
pub async fn explain(
    pool: &Pool,
    sql: &str,
    page: &QueryOptions,
    limits: &ReadOnlyLimits,
) -> Result<ReadOnlyPlan> {
    let sql = crate::adapters::trim_statement(sql);
    if sql.is_empty() {
        return Err(Error::InvalidQuery("Empty query".into()));
    }

    let mut client = pool.get().await?;
    let tx = client.build_transaction().read_only(true).start().await.map_err(db_error)?;
    let result = run_explain(&tx, sql, page, limits).await;
    finish(tx, result).await
}

/// Nada desta transação vale, nem quando deu certo. Se a falha derrubou a
/// conexão, o deadpool a descarta na volta ao pool.
async fn finish<T>(tx: deadpool_postgres::Transaction<'_>, result: Result<T>) -> Result<T> {
    let rollback = tx.rollback().await.map_err(db_error);
    let value = result?;
    rollback?;
    Ok(value)
}

/// SET LOCAL: valem só nesta transação. O texto é todo nosso (números e um
/// identificador escapado), nunca da consulta.
async fn apply_limits(tx: &Transaction<'_>, page: &QueryOptions, limits: &ReadOnlyLimits) -> Result<()> {
    let mut settings = format!(
        "SET LOCAL statement_timeout = {}; SET LOCAL lock_timeout = {};",
        limits.statement_timeout.as_millis(),
        limits.lock_timeout.as_millis()
    );
    if let Some(schema) = page.schema.as_deref() {
        settings.push_str(&format!(" SET LOCAL search_path TO {}, public;", quote_ident(schema)));
    }
    tx.batch_execute(&settings).await.map_err(db_error)
}

/// Erro do servidor com o SQLSTATE (só a mensagem principal: `DETAIL`,
/// `HINT` e `WHERE` ficam de fora, é onde os valores aparecem — RN-10).
fn db_error(error: tokio_postgres::Error) -> Error {
    match error.as_db_error() {
        Some(db) => Error::DbQuery {
            sql_state: db.code().code().to_string(),
            message: db.message().to_string(),
        },
        None => error.into(),
    }
}

async fn run(
    tx: &Transaction<'_>,
    sql: &str,
    page: &QueryOptions,
    limits: &ReadOnlyLimits,
) -> Result<QueryResult> {
    apply_limits(tx, page, limits).await?;

    // Colunas e tipos reais da consulta, sem executar nada
    let probe = tx
        .prepare(&format!("SELECT * FROM (\n{sql}\n) AS __q"))
        .await
        .map_err(db_error)?;
    let columns = columns_from_statement(&probe);

    let (limit, offset) = limits.page(page.limit, page.offset);
    let paged = paged_text_query(sql, columns.len(), limit, offset);

    let started = Instant::now();
    let rows = tx.query(&paged, &[]).await.map_err(db_error)?;
    let execution_time_ms = started.elapsed().as_millis() as u64;

    let has_more = rows.len() as i64 > limit;
    let rows = &rows[..rows.len().min(limit as usize)];

    Ok(QueryResult {
        columns,
        rows: extract_text_rows_typed(rows),
        row_count: rows.len(),
        total_count: None,
        has_more,
        execution_time_ms,
        // O resultado do assistente nunca é editável (RN-16)
        editable_info: None,
    })
}

async fn run_explain(
    tx: &Transaction<'_>,
    sql: &str,
    page: &QueryOptions,
    limits: &ReadOnlyLimits,
) -> Result<ReadOnlyPlan> {
    apply_limits(tx, page, limits).await?;

    // Dentro da subquery, como na execução: só consulta entra, e um
    // `ANALYZE` no começo do texto não vira `EXPLAIN ANALYZE`
    let row = tx
        .query_one(
            &format!("EXPLAIN (FORMAT JSON) SELECT * FROM (\n{sql}\n) AS __q"),
            &[],
        )
        .await
        .map_err(db_error)?;
    let JsonText(plan) = row.try_get(0).map_err(db_error)?;
    Ok(plan_estimate(plan))
}

/// O `EXPLAIN (FORMAT JSON)` devolve uma coluna `json`, que o tokio-postgres
/// não lê como `String` sem a feature do serde. No protocolo binário o `json`
/// é o próprio texto.
struct JsonText(String);

impl<'a> FromSql<'a> for JsonText {
    fn from_sql(
        _: &Type,
        raw: &'a [u8],
    ) -> std::result::Result<Self, Box<dyn std::error::Error + Sync + Send>> {
        Ok(Self(std::str::from_utf8(raw)?.to_string()))
    }

    fn accepts(ty: &Type) -> bool {
        *ty == Type::JSON
    }
}

/// `[{"Plan": {"Total Cost": …, "Plan Rows": …}}]` → a estimativa da raiz.
fn plan_estimate(plan: String) -> ReadOnlyPlan {
    let root = serde_json::from_str::<serde_json::Value>(&plan)
        .ok()
        .and_then(|value| value.get(0)?.get("Plan").cloned());
    let number = |key: &str| root.as_ref().and_then(|root| root.get(key)?.as_f64());
    ReadOnlyPlan {
        estimated_rows: number("Plan Rows"),
        estimated_cost: number("Total Cost"),
        plan,
    }
}

/// A página com cada coluna em `::text`: o Postgres renderiza o valor (como
/// no browse) e nada é decodificado por tipo. As colunas são renomeadas por
/// posição (`__q(c1, c2…)`), então nomes repetidos (`SELECT 1, 1`) não colidem.
/// `limit + 1` descobre o `has_more` sem outra consulta.
fn paged_text_query(sql: &str, column_count: usize, limit: i64, offset: i64) -> String {
    let (select_list, alias) = if column_count == 0 {
        (String::new(), "__q".to_string())
    } else {
        let names: Vec<String> = (1..=column_count).map(|i| format!("c{i}")).collect();
        let select_list = names
            .iter()
            .map(|name| format!("__q.{name}::text"))
            .collect::<Vec<_>>()
            .join(", ");
        (select_list, format!("__q({})", names.join(", ")))
    };
    format!(
        "SELECT {select_list} FROM (\n{sql}\n) AS {alias} LIMIT {} OFFSET {offset}",
        limit + 1
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paged_query_casts_columns_by_position() {
        assert_eq!(
            paged_text_query("SELECT 1, 1", 2, 10, 20),
            "SELECT __q.c1::text, __q.c2::text FROM (\nSELECT 1, 1\n) AS __q(c1, c2) LIMIT 11 OFFSET 20"
        );
        assert_eq!(
            paged_text_query("SELECT", 0, 5, 0),
            "SELECT  FROM (\nSELECT\n) AS __q LIMIT 6 OFFSET 0"
        );
    }

    #[test]
    fn plan_estimate_reads_the_root_node() {
        let plan = r#"[{"Plan": {"Node Type": "Seq Scan", "Total Cost": 35.5, "Plan Rows": 2550}}]"#;
        let estimate = plan_estimate(plan.into());
        assert_eq!(estimate.estimated_rows, Some(2550.0));
        assert_eq!(estimate.estimated_cost, Some(35.5));
        assert_eq!(estimate.plan, plan);

        let unknown = plan_estimate("not json".into());
        assert_eq!((unknown.estimated_rows, unknown.estimated_cost), (None, None));
    }

    #[test]
    fn trailing_comment_cannot_swallow_the_wrapper() {
        let paged = paged_text_query("SELECT 1 -- fim", 1, 10, 0);
        assert!(paged.contains("-- fim\n) AS __q(c1)"), "{paged}");
    }
}

/// E2E contra o Postgres dos testes (localhost:5432, postgres/1234 — ver a
/// memória do projeto). `cargo test --lib postgres::read_only -- --ignored`.
/// Aqui o guard fica **desligado**: o executor é chamado direto, e a escrita
/// tem que ser barrada pelo próprio banco.
#[cfg(test)]
mod e2e {
    use std::time::Duration;

    use super::*;
    use crate::adapters::postgres::PostgresAdapter;
    use crate::adapters::{DatabaseAdapter, ReadOnlyOp};
    use crate::models::{DatabaseType, Server, SslMode};


    fn server() -> Server {
        Server {
            id: Some(1),
            name: "e2e".into(),
            db_type: DatabaseType::Postgres,
            host: "localhost".into(),
            port: 5432,
            username: "postgres".into(),
            password: "1234".into(),
            default_database: None,
            ssl_mode: SslMode::Disable,
            ssl_root_cert: None,
            connection_uri: None,
            scope_databases: None,
            scope_schemas: None,
            group_id: None,
            assistant_sample_values: false,
            created_at: 0,
        }
    }

    /// Banco novo com uma tabela, uma sequência, uma função que escreve e um
    /// schema extra.
    /// Um banco por teste: os testes rodam em paralelo.
    async fn fixture(database: &str) -> PostgresAdapter {
        let admin = PostgresAdapter::new(&server(), "postgres").unwrap();
        admin
            .execute_statement(&format!("DROP DATABASE IF EXISTS {database} WITH (FORCE)"))
            .await
            .unwrap();
        admin.execute_statement(&format!("CREATE DATABASE {database}")).await.unwrap();

        let adapter = PostgresAdapter::new(&server(), database).unwrap();
        adapter
            .execute_transaction(vec![
                "CREATE TABLE users (id serial PRIMARY KEY, name text, active boolean, tags text[])"
                    .into(),
                "INSERT INTO users (name, active, tags) \
                 SELECT 'user ' || g, g % 2 = 0, ARRAY['a', 'b'] FROM generate_series(1, 25) g"
                    .into(),
                "CREATE SEQUENCE counter".into(),
                "CREATE FUNCTION sneaky_write() RETURNS int LANGUAGE sql VOLATILE \
                 AS $$ INSERT INTO users (name) VALUES ('sneaky') RETURNING id $$"
                    .into(),
                "CREATE SCHEMA tenant_1".into(),
                "CREATE TABLE tenant_1.orders (id int)".into(),
                "INSERT INTO tenant_1.orders VALUES (7)".into(),
            ])
            .await
            .unwrap();
        adapter
    }

    async fn drop_fixture(database: &str) {
        let admin = PostgresAdapter::new(&server(), "postgres").unwrap();
        admin
            .execute_statement(&format!("DROP DATABASE IF EXISTS {database} WITH (FORCE)"))
            .await
            .unwrap();
    }

    async fn read(adapter: &PostgresAdapter, sql: &str) -> Result<QueryResult> {
        adapter
            .execute_read_only(&ReadOnlyOp::Sql(sql.into()), QueryOptions::default())
            .await
    }

    async fn user_count(adapter: &PostgresAdapter) -> String {
        adapter
            .execute_query("SELECT count(*) FROM users", QueryOptions::default())
            .await
            .unwrap()
            .rows[0][0]
            .clone()
            .unwrap()
    }

    #[tokio::test]
    #[ignore]
    async fn e2e_read_only_reads_pages_and_never_writes() {
        const DATABASE: &str = "octapus_db_e2e_read_only";
        let adapter = fixture(DATABASE).await;

        // Leitura normal: valores em texto, colunas com o tipo real, paginação
        let result = adapter
            .execute_read_only(
                &ReadOnlyOp::Sql("SELECT id, name, active, tags FROM users ORDER BY id;".into()),
                QueryOptions { limit: 10, offset: 20, ..QueryOptions::default() },
            )
            .await
            .unwrap();
        assert_eq!(result.row_count, 5);
        assert!(!result.has_more);
        assert_eq!(result.columns[2].type_name, "bool");
        assert_eq!(result.rows[0], vec![
            Some("21".into()),
            Some("user 21".into()),
            Some("false".into()),
            Some("{a,b}".into()),
        ]);
        assert!(result.editable_info.is_none());

        let first_page = adapter
            .execute_read_only(
                &ReadOnlyOp::Sql("SELECT * FROM users".into()),
                QueryOptions { limit: 10, ..QueryOptions::default() },
            )
            .await
            .unwrap();
        assert_eq!(first_page.row_count, 10);
        assert!(first_page.has_more);

        // Nomes repetidos e comentário no fim
        let result = read(&adapter, "SELECT 1, 1 -- fim").await.unwrap();
        assert_eq!(result.rows, vec![vec![Some("1".into()), Some("1".into())]]);

        // Schema no search_path, só nesta transação
        let result = adapter
            .execute_read_only(
                &ReadOnlyOp::Sql("SELECT id FROM orders".into()),
                QueryOptions { schema: Some("tenant_1".into()), ..QueryOptions::default() },
            )
            .await
            .unwrap();
        assert_eq!(result.rows, vec![vec![Some("7".into())]]);
        assert!(read(&adapter, "SELECT id FROM orders").await.is_err());

        // Escritas, com o guard desligado: o banco recusa todas
        let before = user_count(&adapter).await;
        for sql in [
            "SELECT 1; DELETE FROM users",
            "SELECT 1) AS x; COMMIT; DELETE FROM users; SELECT * FROM (SELECT 1",
            "DELETE FROM users RETURNING id",
            "WITH gone AS (DELETE FROM users RETURNING id) SELECT * FROM gone",
            "SELECT sneaky_write()",
            "SELECT nextval('counter')",
            "SELECT * FROM users FOR UPDATE",
            "SELECT * INTO copied FROM users",
            "CREATE TABLE evil (id int)",
        ] {
            assert!(read(&adapter, sql).await.is_err(), "devia falhar: {sql}");
        }
        assert_eq!(user_count(&adapter).await, before, "nenhuma escrita pode ter valido");
        let tables = adapter
            .execute_query(
                "SELECT count(*) FROM pg_tables WHERE tablename IN ('copied', 'evil')",
                QueryOptions::default(),
            )
            .await
            .unwrap();
        assert_eq!(tables.rows[0][0].as_deref(), Some("0"));
        let counter = adapter
            .execute_query("SELECT last_value, is_called FROM counter", QueryOptions::default())
            .await
            .unwrap();
        assert_eq!(counter.rows[0][1].as_deref(), Some("f"), "a sequência não pode ter andado");

        // A conexão volta ao pool sem transação aberta nem READ ONLY sobrando
        let after = adapter
            .execute_query("SHOW transaction_read_only", QueryOptions::default())
            .await
            .unwrap();
        assert_eq!(after.rows[0][0].as_deref(), Some("off"));

        drop(adapter);
        drop_fixture(DATABASE).await;
    }

    #[tokio::test]
    #[ignore]
    async fn e2e_read_only_times_out() {
        const DATABASE: &str = "octapus_db_e2e_read_only_timeout";
        let adapter = fixture(DATABASE).await;
        let limits = ReadOnlyLimits {
            statement_timeout: Duration::from_millis(200),
            ..ReadOnlyLimits::default()
        };

        let started = Instant::now();
        let error = execute(
            &adapter.pool,
            "SELECT pg_sleep(5)",
            &QueryOptions::default(),
            &limits,
            &adapter.active_queries,
        )
        .await
        .unwrap_err();
        assert!(started.elapsed() < Duration::from_secs(3));
        assert!(
            error.to_string().contains(crate::adapters::STATEMENT_TIMEOUT_MESSAGE),
            "{error}"
        );

        drop(adapter);
        drop_fixture(DATABASE).await;
    }
}
