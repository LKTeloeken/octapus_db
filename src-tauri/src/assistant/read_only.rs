//! A execução de uma consulta do assistente: o guard e depois o executor
//! só-leitura do adapter. O guard é a primeira barreira; a segunda é o
//! próprio banco (`adapters/read_only.rs`), que segura sozinha.

use std::time::Instant;

use crate::adapters::{DatabaseAdapter, ReadOnlyOp};
use crate::error::Result;
use crate::models::{DatabaseType, QueryOptions, QueryResult};

use super::guard;

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
