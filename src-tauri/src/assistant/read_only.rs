//! A execução de uma consulta do assistente: o guard e depois o executor
//! só-leitura do adapter. O guard é a primeira barreira; a segunda é o
//! próprio banco (`adapters/read_only.rs`), que segura sozinha.

use std::time::Instant;

use crate::adapters::{DatabaseAdapter, ReadOnlyOp};
use crate::error::Result;
use crate::models::{DatabaseType, QueryOptions, QueryResult, ReadOnlyPlan};

use super::guard;

/// O preflight: o guard e depois o plano da consulta, sem executá-la. Uma
/// falha aqui (coluna que não existe, tipo errado) já vai para a correção.
#[allow(dead_code)] // chamada pelo orquestrador do turno (B6)
pub async fn preflight(
    adapter: &dyn DatabaseAdapter,
    db_type: DatabaseType,
    op: &ReadOnlyOp,
    page: QueryOptions,
) -> Result<ReadOnlyPlan> {
    guard::check(op, db_type)?;

    let started = Instant::now();
    let result = adapter.explain_read_only(op, page).await;
    match &result {
        Ok(_) => log::info!("assistente: preflight ok em {} ms", started.elapsed().as_millis()),
        Err(error) => log::info!("assistente: preflight falhou ({})", error.code()),
    }
    result
}

#[allow(dead_code)] // chamada pelo orquestrador do turno (B6)
pub async fn execute(
    adapter: &dyn DatabaseAdapter,
    db_type: DatabaseType,
    op: &ReadOnlyOp,
    page: QueryOptions,
) -> Result<QueryResult> {
    guard::check(op, db_type)?;

    let started = Instant::now();
    let result = adapter.execute_read_only(op, page).await;
    match &result {
        Ok(result) => log::info!(
            "assistente: leitura com {} linha(s) em {} ms",
            result.row_count,
            started.elapsed().as_millis()
        ),
        Err(error) => log::info!("assistente: leitura falhou ({})", error.code()),
    }
    result
}

/// O caminho inteiro (guard + executor) contra bancos de verdade. As escritas
/// param no guard (`GUARD_REJECTED`); as que o guard não vê, no banco — os
/// testes de cada executor (`adapters/*/read_only.rs`) cobrem o banco sozinho.
#[cfg(test)]
mod e2e {
    use std::time::Instant;

    use super::*;
    use crate::adapters::postgres::PostgresAdapter;
    use crate::adapters::sqlite::SqliteAdapter;
    use crate::models::{Server, SslMode};

    const WRITES: &[&str] = &[
        "DELETE FROM users",
        "SELECT 1; DELETE FROM users",
        "WITH gone AS (DELETE FROM users RETURNING id) SELECT * FROM gone",
        "SELECT * INTO copied FROM users",
        "DROP TABLE users",
    ];

    fn server(db_type: DatabaseType, connection_uri: Option<String>) -> Server {
        Server {
            id: Some(1),
            name: "e2e".into(),
            db_type,
            host: "localhost".into(),
            port: 5432,
            username: "postgres".into(),
            password: "1234".into(),
            default_database: None,
            ssl_mode: SslMode::Disable,
            ssl_root_cert: None,
            connection_uri,
            scope_databases: None,
            scope_schemas: None,
            group_id: None,
            created_at: 0,
        }
    }

    async fn assert_guarded(adapter: &dyn DatabaseAdapter, db_type: DatabaseType) {
        let count = || async {
            execute(adapter, db_type, &ReadOnlyOp::Sql("SELECT count(*) FROM users".into()), QueryOptions::default())
                .await
                .unwrap()
                .rows[0][0]
                .clone()
        };
        let before = count().await;
        assert_eq!(before.as_deref(), Some("3"));

        for sql in WRITES {
            let error = execute(adapter, db_type, &ReadOnlyOp::Sql((*sql).into()), QueryOptions::default())
                .await
                .unwrap_err();
            assert_eq!(error.code(), "GUARD_REJECTED", "{db_type:?}: {sql}");
        }
        assert_eq!(count().await, before);
    }

    #[tokio::test]
    async fn e2e_sqlite_guard_and_database() {
        let path = std::env::temp_dir().join("octapus_db_assistant_read_only.db");
        let _ = std::fs::remove_file(&path);
        rusqlite::Connection::open(&path)
            .unwrap()
            .execute_batch(
                "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT);
                 INSERT INTO users (name) VALUES ('a'), ('b'), ('c');",
            )
            .unwrap();
        let server = server(DatabaseType::Sqlite, Some(path.to_string_lossy().into_owned()));
        let adapter = SqliteAdapter::new(&server, "main").unwrap();

        assert_guarded(&adapter, DatabaseType::Sqlite).await;

        drop(adapter);
        let _ = std::fs::remove_file(path);
    }

    /// Preflight acha o erro sem executar, e nenhum valor do banco sai no erro
    /// sanitizado (RN-10).
    async fn assert_preflight_and_canaries(adapter: &PostgresAdapter) {
        use crate::assistant::failure::{classify, sanitize, FailureClass};

        let pg = DatabaseType::Postgres;
        let sql = |text: &str| ReadOnlyOp::Sql(text.into());
        adapter
            .execute_transaction(vec![
                "CREATE TABLE secrets (label text, born text, data text)".into(),
                "INSERT INTO secrets VALUES ('CANARY_7f3a', '2026-13-45', '{\"k\": CANARY_9b1c}')"
                    .into(),
                "CREATE FUNCTION sneaky_write() RETURNS int LANGUAGE sql VOLATILE \
                 AS $$ INSERT INTO users (name) VALUES ('sneaky') RETURNING id $$"
                    .into(),
            ])
            .await
            .unwrap();

        let plan = preflight(adapter, pg, &sql("SELECT name FROM users WHERE id > 1"), QueryOptions::default())
            .await
            .unwrap();
        assert!(plan.estimated_rows.is_some() && plan.estimated_cost.is_some(), "{plan:?}");

        // Coluna inexistente: o planejador acha, nada roda
        let query = "SELECT nome FROM users";
        let error = preflight(adapter, pg, &sql(query), QueryOptions::default()).await.unwrap_err();
        assert_eq!(classify(&error), Some(FailureClass::UnknownObject));
        assert_eq!(
            sanitize(&error, query).as_deref(),
            Some("42703: column \"nome\" does not exist")
        );

        // Função que escreve e consulta lenta: planejar não executa
        let started = Instant::now();
        preflight(adapter, pg, &sql("SELECT sneaky_write(), pg_catalog.now()"), QueryOptions::default())
            .await
            .unwrap();
        assert!(started.elapsed().as_secs() < 2);
        let names = execute(adapter, pg, &sql("SELECT count(*) FROM users WHERE name = 'sneaky'"), QueryOptions::default())
            .await
            .unwrap();
        assert_eq!(names.rows[0][0].as_deref(), Some("0"));

        // Valores do banco no erro: nenhum sai
        for (query, class) in [
            ("SELECT label::int FROM secrets", FailureClass::TypeMismatch),
            ("SELECT born::date FROM secrets", FailureClass::TypeMismatch),
            ("SELECT data::json FROM secrets", FailureClass::TypeMismatch),
            ("SELECT label::uuid FROM secrets", FailureClass::TypeMismatch),
        ] {
            let error = execute(adapter, pg, &sql(query), QueryOptions::default()).await.unwrap_err();
            assert_eq!(classify(&error), Some(class), "{query}: {error}");
            let sanitized = sanitize(&error, query).unwrap();
            for canary in ["CANARY_7f3a", "CANARY_9b1c", "2026-13-45"] {
                assert!(!sanitized.contains(canary), "{canary} vazou em: {sanitized}");
            }
        }
    }

    /// Postgres dos e2e (localhost:5432, postgres/1234).
    /// `cargo test --lib assistant::read_only -- --ignored`
    #[tokio::test]
    #[ignore]
    async fn e2e_postgres_guard_and_database() {
        const DATABASE: &str = "octapus_db_e2e_assistant_read_only";
        let admin = PostgresAdapter::new(&server(DatabaseType::Postgres, None), "postgres").unwrap();
        admin
            .execute_statement(&format!("DROP DATABASE IF EXISTS {DATABASE} WITH (FORCE)"))
            .await
            .unwrap();
        admin.execute_statement(&format!("CREATE DATABASE {DATABASE}")).await.unwrap();

        let adapter = PostgresAdapter::new(&server(DatabaseType::Postgres, None), DATABASE).unwrap();
        adapter
            .execute_transaction(vec![
                "CREATE TABLE users (id serial PRIMARY KEY, name text)".into(),
                "INSERT INTO users (name) VALUES ('a'), ('b'), ('c')".into(),
            ])
            .await
            .unwrap();

        assert_guarded(&adapter, DatabaseType::Postgres).await;
        assert_preflight_and_canaries(&adapter).await;
        // O guard recusa o que a transação READ ONLY deixaria passar
        let error = execute(
            &adapter,
            DatabaseType::Postgres,
            &ReadOnlyOp::Sql("SELECT pg_advisory_lock(1)".into()),
            QueryOptions::default(),
        )
        .await
        .unwrap_err();
        assert_eq!(error.code(), "GUARD_REJECTED");

        drop(adapter);
        admin
            .execute_statement(&format!("DROP DATABASE IF EXISTS {DATABASE} WITH (FORCE)"))
            .await
            .unwrap();
    }
}
