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

    /// Redis dos e2e (localhost:6380, sem auth; dá FLUSHDB no db 1 — o db 0
    /// é do e2e do adapter).
    /// `cargo test --lib assistant::read_only -- --ignored`
    #[tokio::test]
    #[ignore = "requires local redis on localhost:6380 (no auth)"]
    async fn e2e_redis_context_guard_foreach_and_canaries() {
        use crate::adapters::redisdb::RedisAdapter;
        use crate::assistant::failure::{classify, sanitize, FailureClass};
        use crate::assistant::key_context;
        use crate::assistant::schema_context::ContextRequest;

        let redis = DatabaseType::Redis;
        let op = |text: &str| ReadOnlyOp::RedisPlan(text.into());
        let mut server = server(redis, None);
        server.port = 6380;
        server.username = String::new();
        server.password = String::new();
        let adapter = RedisAdapter::new(&server, "1").unwrap();
        adapter.execute_statement("FLUSHDB").await.unwrap();

        // Chaves e valores com canários: nenhum pode sair do app
        for i in 1..=30 {
            adapter
                .execute_statement(&format!("HSET user:{} name CANARY_name_{i} email user{i}@canary.example", 48200 + i))
                .await
                .unwrap();
        }
        for i in 1..=5 {
            adapter.execute_statement(&format!("RPUSH user:{}:cart CANARY_item_{i}", 48200 + i)).await.unwrap();
        }
        adapter
            .execute_statement("SET session:550e8400-e29b-41d4-a716-446655440000 CANARY_token")
            .await
            .unwrap();
        adapter.execute_statement("SET reset:ana.canary@example.com 1").await.unwrap();
        adapter.execute_statement("ZADD leaderboard 10 CANARY_ana 7 CANARY_bia 3 CANARY_caio").await.unwrap();
        let members: Vec<String> = (0..1500).map(|i| format!("m{i}")).collect();
        adapter.execute_statement(&format!("SADD big {}", members.join(" "))).await.unwrap();
        let dbsize = || async {
            adapter.execute_query("DBSIZE", QueryOptions::default()).await.unwrap().rows[0][0].clone()
        };
        let before = dbsize().await;

        // Contexto: padrões e tipos, sem chave completa nem valor (RN-09)
        let context = key_context::build(
            &adapter,
            ContextRequest {
                schemas: &[],
                question: "quantos usuários há?",
                history: &[],
                context_window: 200_000,
            },
        )
        .await
        .unwrap();
        assert!(context.ddl.contains("\"user:{id}\" hash, 30 keys"), "{}", context.ddl);
        assert!(context.ddl.contains("\"user:{id}:cart\" list, 5 keys"), "{}", context.ddl);
        assert!(context.ddl.contains("\"leaderboard\" zset, 1 key"), "{}", context.ddl);
        for canary in ["CANARY", "canary", "48201", "550e8400", "example"] {
            assert!(!context.ddl.contains(canary), "{canary} vazou no contexto:\n{}", context.ddl);
        }

        // Guard: KEYS, scripts e escritas param antes do banco
        for text in [
            r#"{"steps":[{"id":"a","command":"KEYS","args":["*"],"foreach":null}],"output":"a"}"#,
            r#"{"steps":[{"id":"a","command":"EVAL","args":["return redis.call('FLUSHDB')","0"],"foreach":null}],"output":"a"}"#,
            r#"{"steps":[{"id":"a","command":"FLUSHDB","args":[],"foreach":null}],"output":"a"}"#,
            r#"{"steps":[{"id":"a","command":"SCAN","args":["0","MATCH","user:*"],"foreach":null},
                {"id":"b","command":"DEL","args":["$item"],"foreach":"a"}],"output":"b"}"#,
            r#"{"steps":[{"id":"a","command":"SET","args":["x","1"],"foreach":null}],"output":"a"}"#,
        ] {
            let error = execute(&adapter, redis, &op(text), QueryOptions::default()).await.unwrap_err();
            assert_eq!(error.code(), "GUARD_REJECTED", "{text}");
        }
        assert_eq!(dbsize().await, before);

        // foreach: SCAN dos usuários e o nome de cada um, com total e páginas
        let names = r#"{"steps":[
            {"id":"users","command":"SCAN","args":["0","MATCH","user:*","TYPE","hash"],"foreach":null},
            {"id":"names","command":"HGET","args":["$item","name"],"foreach":"users"}],"output":"names"}"#;
        let page = execute(&adapter, redis, &op(names), QueryOptions { limit: 10, ..QueryOptions::default() })
            .await
            .unwrap();
        assert_eq!((page.row_count, page.has_more, page.total_count), (10, true, Some(30)));
        let columns: Vec<&str> = page.columns.iter().map(|column| column.name.as_str()).collect();
        assert_eq!(columns, ["key", "name"]);
        assert!(page.rows.iter().all(|row| row[1].as_deref().is_some_and(|name| name.starts_with("CANARY_name_"))));
        let last = execute(&adapter, redis, &op(names), QueryOptions { limit: 10, offset: 25, ..QueryOptions::default() })
            .await
            .unwrap();
        assert_eq!((last.row_count, last.has_more), (5, false));

        // HGETALL por item abre uma coluna por campo; ranking direto no servidor
        let all = r#"{"steps":[
            {"id":"users","command":"SCAN","args":["0","MATCH","user:*","TYPE","hash"],"foreach":null},
            {"id":"all","command":"HGETALL","args":["$item"],"foreach":"users"}],"output":"all"}"#;
        let result = execute(&adapter, redis, &op(all), QueryOptions::default()).await.unwrap();
        let columns: Vec<&str> = result.columns.iter().map(|column| column.name.as_str()).collect();
        assert_eq!(columns, ["key", "name", "email"]);
        let top = r#"{"steps":[{"id":"top","command":"ZRANGE","args":["leaderboard","0","1","REV","WITHSCORES"],"foreach":null}],"output":"top"}"#;
        let result = execute(&adapter, redis, &op(top), QueryOptions::default()).await.unwrap();
        assert_eq!(result.rows, [vec![Some("CANARY_ana".into()), Some("10".into())], vec![Some("CANARY_bia".into()), Some("7".into())]]);

        // Preflight: aridade errada sem executar; o plano certo vira resumo
        let wrong = r#"{"steps":[{"id":"a","command":"HGET","args":["user:48201"],"foreach":null}],"output":"a"}"#;
        let error = preflight(&adapter, redis, &op(wrong), QueryOptions::default()).await.unwrap_err();
        assert_eq!(classify(&error), Some(FailureClass::Syntax));
        assert_eq!(
            sanitize(&error, wrong).as_deref(),
            Some("ERR: wrong number of arguments for 'hget' command")
        );
        let plan = preflight(&adapter, redis, &op(names), QueryOptions::default()).await.unwrap();
        assert_eq!(plan.plan, "SCAN → foreach HGET");

        // WRONGTYPE: o SCAN sem TYPE pega as listas também; nada do banco no erro
        let mixed = r#"{"steps":[
            {"id":"users","command":"SCAN","args":["0","MATCH","user:*"],"foreach":null},
            {"id":"names","command":"HGET","args":["$item","name"],"foreach":"users"}],"output":"names"}"#;
        let error = execute(&adapter, redis, &op(mixed), QueryOptions { limit: 1000, ..QueryOptions::default() })
            .await
            .unwrap_err();
        assert_eq!(classify(&error), Some(FailureClass::TypeMismatch), "{error:?}");
        let sanitized = sanitize(&error, mixed).unwrap();
        assert_eq!(sanitized, "WRONGTYPE: Operation against a key holding the wrong kind of value");

        // Teto: foreach no meio do plano sobre 1500 itens é limitação do paradigma
        let fan_out = r#"{"steps":[
            {"id":"members","command":"SMEMBERS","args":["big"],"foreach":null},
            {"id":"exists","command":"EXISTS","args":["$item"],"foreach":"members"},
            {"id":"types","command":"TYPE","args":["$item"],"foreach":"exists"}],"output":"types"}"#;
        let error = execute(&adapter, redis, &op(fan_out), QueryOptions::default()).await.unwrap_err();
        assert_eq!(classify(&error), Some(FailureClass::ParadigmLimitation), "{error:?}");
        assert!(sanitize(&error, fan_out).unwrap().contains("more than 1000 items"));

        assert_eq!(dbsize().await, before);
        adapter.execute_statement("FLUSHDB").await.unwrap();
    }

    /// Mongo dos e2e (localhost:27017, sem auth).
    /// `cargo test --lib assistant::read_only -- --ignored`
    #[tokio::test]
    #[ignore = "requires local mongodb on localhost:27017 (no auth)"]
    async fn e2e_mongo_guard_preflight_and_canaries() {
        use mongodb::bson::{doc, Document};

        use crate::adapters::mongo::MongoAdapter;
        use crate::assistant::failure::{classify, sanitize, FailureClass};

        const DATABASE: &str = "octapus_e2e_assistant_guarded";
        let mongo = DatabaseType::Mongodb;
        let op = |text: &str| ReadOnlyOp::Mongo(text.into());
        let mut server = server(mongo, None);
        server.port = 27017;
        server.username = String::new();
        server.password = String::new();
        let adapter = MongoAdapter::new(&server, DATABASE).await.unwrap();

        let client = mongodb::Client::with_uri_str("mongodb://localhost:27017").await.unwrap();
        let db = client.database(DATABASE);
        db.drop().await.unwrap();
        db.collection::<Document>("users")
            .insert_many((0..3).map(|i| doc! { "name": format!("user {i}"), "label": "CANARY_7f3a", "born": "2026-13-45" }))
            .await
            .unwrap();
        let count = || async { db.collection::<Document>("users").count_documents(doc! {}).await.unwrap() };

        // Escritas e JavaScript param no guard
        for text in [
            r#"{"collection":"users","operation":"aggregate","pipeline":[{"$out":"copied"}]}"#,
            r#"{"collection":"users","operation":"aggregate","pipeline":[{"$merge":{"into":"users"}}]}"#,
            r#"{"collection":"users","operation":"deleteMany","filter":{}}"#,
            r#"{"collection":"users","operation":"find","filter":{"$where":"sleep(1000) || true"}}"#,
            r#"{"collection":"users","operation":"aggregate","pipeline":[{"$currentOp":{}}]}"#,
        ] {
            let error = execute(&adapter, mongo, &op(text), QueryOptions::default()).await.unwrap_err();
            assert_eq!(error.code(), "GUARD_REJECTED", "{text}");
        }
        assert_eq!(count().await, 3);
        assert_eq!(db.list_collection_names().await.unwrap(), ["users"]);

        // Preflight: coleção que não existe e operador desconhecido, sem rodar
        let query = r#"{"collection":"usuarios","operation":"find"}"#;
        let error = preflight(&adapter, mongo, &op(query), QueryOptions::default()).await.unwrap_err();
        assert_eq!(classify(&error), Some(FailureClass::UnknownObject));
        assert_eq!(
            sanitize(&error, query).as_deref(),
            Some("NamespaceNotFound (26): collection \"usuarios\" does not exist")
        );
        let query = r#"{"collection":"users","operation":"find","filter":{"name":{"$likee":"a"}}}"#;
        let error = preflight(&adapter, mongo, &op(query), QueryOptions::default()).await.unwrap_err();
        assert_eq!(classify(&error), Some(FailureClass::Syntax), "{error:?}");
        assert!(sanitize(&error, query).unwrap().contains("$likee"));

        // Valores do banco no erro: nenhum sai
        for query in [
            r#"{"collection":"users","operation":"aggregate","pipeline":[{"$project":{"n":{"$toInt":"$label"}}}]}"#,
            r#"{"collection":"users","operation":"aggregate","pipeline":[{"$project":{"d":{"$dateFromString":{"dateString":"$born"}}}}]}"#,
            r#"{"collection":"users","operation":"aggregate","pipeline":[{"$project":{"o":{"$toObjectId":"$label"}}}]}"#,
            r#"{"collection":"users","operation":"aggregate","pipeline":[{"$project":{"x":{"$add":["$label",1]}}}]}"#,
        ] {
            let error = execute(&adapter, mongo, &op(query), QueryOptions::default()).await.unwrap_err();
            let class = classify(&error);
            assert!(matches!(class, Some(FailureClass::TypeMismatch | FailureClass::Syntax | FailureClass::RuntimeError)), "{query}: {error:?}");
            let sanitized = sanitize(&error, query).unwrap();
            for canary in ["CANARY_7f3a", "2026-13-45", "user 0"] {
                assert!(!sanitized.contains(canary), "{canary} vazou em: {sanitized}");
            }
        }

        // A leitura passa e a página respeita o teto
        let read = execute(
            &adapter,
            mongo,
            &op(r#"{"collection":"users","operation":"find","sort":{"name":1},"projection":{"_id":0,"name":1}}"#),
            QueryOptions { limit: 2, ..QueryOptions::default() },
        )
        .await
        .unwrap();
        assert_eq!((read.row_count, read.has_more), (2, true));
        assert_eq!(read.rows[0][0].as_deref(), Some("user 0"));

        db.drop().await.unwrap();
    }
}
