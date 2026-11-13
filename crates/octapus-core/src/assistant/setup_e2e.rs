//! Turnos de verdade contra uma API do assistente (a local, com a chave de
//! teste): convite → termo → conversa → contexto de um banco descartável →
//! turno com o modelo real. Gastam tokens; só rodam com as variáveis:
//!
//! ```text
//! OCTAPUS_E2E_ASSISTANT_URL=http://localhost:8787 OCTAPUS_E2E_INVITE=<convite> \
//!   cargo test -p octapus-core --lib assistant::setup::e2e -- --ignored --nocapture
//! ```
//!
//! O de Mongo usa o banco dos e2e em localhost:27017 e o de Redis o de
//! localhost:6380 (db 2; o 0 e o 1 são dos outros e2e), os dois sem auth.
//! A prova de privacidade (B10) cresce a partir daqui: `privacy_e2e`.

use std::sync::Arc;

use parking_lot::Mutex;

use async_trait::async_trait;

use super::*;
use crate::adapters::mongo::MongoAdapter;
use crate::assistant::backend::{AttemptProgress, Generation, ModelBackend};
use crate::assistant::language::Detected;
use crate::assistant::messages::AttemptRequest;
use crate::assistant::{CancelFlag, ReplySlot};
use crate::models::{AssistantTurnStatus, AssistantTurnSummary, AttemptOutcome};
use crate::adapters::redisdb::RedisAdapter;
use crate::adapters::sqlite::SqliteAdapter;
use crate::models::{Server, SslMode};
use crate::services::{CatalogEntry, CatalogService, SourceOpener};
use crate::storage::vault;

/// Participante com o termo aceito e uma conversa nova do paradigma. O
/// `assistant` guarda os outcomes (a execução manual completa o deles) e o
/// `sent` tudo que os turnos mandaram à API, para a prova de privacidade.
pub(super) struct Participant {
    pub(super) client: ApiClient,
    pub(super) token: String,
    pub(super) conversation_id: String,
    pub(super) assistant: AssistantService,
    pub(super) sent: Mutex<Vec<String>>,
}

pub(super) async fn participant(db_type: DatabaseType) -> Option<Participant> {
    let (Ok(url), Ok(invite)) = (
        std::env::var("OCTAPUS_E2E_ASSISTANT_URL"),
        std::env::var("OCTAPUS_E2E_INVITE"),
    ) else {
        eprintln!("sem OCTAPUS_E2E_ASSISTANT_URL/OCTAPUS_E2E_INVITE: nada a fazer");
        return None;
    };
    vault::init_for_tests();

    let client = ApiClient::new(&url).unwrap();
    let token = client.redeem(&invite, Some("octapus e2e")).await.unwrap().token;
    let me = client.me(&token).await.unwrap();
    if let Some(current) = me.consent.current_version.as_deref() {
        if me.consent.accepted_version.as_deref() != Some(current) {
            client.consent_accept(&token, current).await.unwrap();
        }
    }
    let paradigm = prompts::paradigm(db_type).unwrap();
    let summary = client.create_conversation(&token, paradigm, None, Some("e2e")).await.unwrap();
    Some(Participant {
        client,
        token,
        conversation_id: summary.id,
        assistant: AssistantService::from_env(),
        sent: Mutex::default(),
    })
}

pub(super) fn server(db_type: DatabaseType, port: u16, connection_uri: Option<String>) -> Server {
    Server {
        id: Some(1),
        name: "e2e".into(),
        db_type,
        host: "localhost".into(),
        port,
        username: String::new(),
        password: String::new(),
        default_database: None,
        ssl_mode: SslMode::Disable,
        ssl_root_cert: None,
        connection_uri,
        scope_databases: None,
        scope_schemas: None,
        group_id: None,
        assistant_sample_values: false,
        display_timezone: None,
        created_at: 0,
    }
}

/// De onde vem a estrutura que vai ao modelo, como no `structure_context`
pub(super) enum Structure<'a> {
    Catalog(&'a CatalogService, &'a Arc<CatalogEntry>),
    Keys,
}

/// Catálogo de um banco descartável, num diretório temporário
pub(super) async fn open_catalog(server: &Server, database: &str) -> (CatalogService, Arc<CatalogEntry>, std::path::PathBuf) {
    let catalogs = CatalogService::default();
    let dir = std::env::temp_dir().join(format!("octapus-assistant-e2e-{}-{database}", std::process::id()));
    catalogs.init(dir.clone(), Arc::new(|_| {}));
    let opener_server = server.clone();
    let opener_database = database.to_string();
    let opener: SourceOpener = Arc::new(move || {
        let server = opener_server.clone();
        let database = opener_database.clone();
        Box::pin(async move { crate::adapters::create_catalog_source(&server, &database).await })
    });
    let entry = catalogs.open(1, database, "e2e".into(), opener).await;
    (catalogs, entry, dir)
}

/// UUID v4 para a idempotência do turno
pub(super) fn client_turn_id() -> String {
    let bits: u128 = (rand::random::<u128>() & !(0xf << 76) | (0x4 << 76)) & !(0x3 << 62) | (0x2 << 62);
    let hex = format!("{bits:032x}");
    format!("{}-{}-{}-{}-{}", &hex[..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..])
}

/// Um turno com execução automática, com o histórico da conversa;
/// imprime os eventos e devolve o fim e as linhas impressas.
pub(super) async fn ask(
    participant: &Participant,
    db: &AdapterDatabase,
    structure: Structure<'_>,
    question: &str,
) -> (AssistantTurnFinish, Vec<String>) {
    ask_with(participant, db, structure, question, &[]).await
}

/// [`ask`] com as menções `@` da pergunta
pub(super) async fn ask_with(
    participant: &Participant,
    db: &AdapterDatabase,
    structure: Structure<'_>,
    question: &str,
    mentions: &[AssistantMention],
) -> (AssistantTurnFinish, Vec<String>) {
    ask_full(participant, db, structure, question, mentions, false).await
}

/// [`ask_with`] escolhendo o interruptor dos valores de exemplo do servidor
pub(super) async fn ask_full(
    participant: &Participant,
    db: &AdapterDatabase,
    structure: Structure<'_>,
    question: &str,
    mentions: &[AssistantMention],
    sample_values: bool,
) -> (AssistantTurnFinish, Vec<String>) {
    let Participant {
        client,
        token,
        conversation_id,
        assistant,
        sent,
    } = participant;
    let conversation = client.conversation(token, conversation_id).await.unwrap();
    let model = ModelProfile::from(&conversation.summary.model);
    let history = turn::history_from(&conversation);
    let past_questions: Vec<String> = history.iter().map(|turn| turn.question.clone()).collect();
    let schemas = conversation_schemas(db.db_type, None);
    let request = ContextRequest {
        schemas: &schemas,
        question,
        history: &past_questions,
        context_window: model.context_window,
        mentions,
        past_mentions: &[],
        sample_values,
    };
    let (catalogs, entry) = match structure {
        Structure::Catalog(catalogs, entry) => (Some(catalogs), Some(Arc::clone(entry))),
        Structure::Keys => (None, None),
    };
    let fallback = CatalogService::default();
    let catalogs = catalogs.unwrap_or(&fallback);
    let schema = structure_context(catalogs, entry.as_ref(), db.db_type, db.adapter.as_ref(), request)
        .await
        .unwrap();
    println!("--- {question}\n{}", schema.ddl);

    let setup = TurnSetup {
        conversation_id: conversation_id.clone(),
        client_turn_id: client_turn_id(),
        question: question.into(),
        language: language::detect(question),
        prompt: prompts::for_db(db.db_type).unwrap(),
        db_type: db.db_type,
        expected_language: query_language(db.db_type),
        schema,
        history,
        model,
        auto_execute: true,
        sample_values: sample_values_option(db.db_type, sample_values),
        // Ninguém responde nos testes de turno real: a pergunta encerra
        reply_timeout: std::time::Duration::from_secs(1),
        page: QueryOptions {
            limit: FIRST_PAGE_ROWS,
            ..QueryOptions::default()
        },
    };
    let api = Recording {
        api: Intermediary { client, token, assistant },
        sent,
    };
    let lookup = RequestedLookup {
        catalogs,
        entry,
        adapter: Arc::clone(&db.adapter),
        db_type: db.db_type,
        scope: &schemas,
        sample_values,
    };
    let ports = TurnPorts {
        backend: &api,
        journal: &api,
        db,
        structure: &lookup,
    };
    let events: Mutex<Vec<String>> = Mutex::default();
    let finish = turn::run_turn(
        &ports,
        &setup,
        &|event| {
            let line = match &event {
                AssistantTurnEvent::Delta { .. } => return,
                AssistantTurnEvent::Output { output, .. } => format!("output: {:?} {:?}", output.kind, output.query),
                AssistantTurnEvent::Result { result, .. } => format!("result: {} linha(s) {:?}", result.row_count, result.rows),
                other => format!("{other:?}"),
            };
            events.lock().push(line);
        },
        &CancelFlag::default(),
        &ReplySlot::default(),
    )
    .await
    .unwrap();

    let events = events.into_inner();
    for line in &events {
        println!("{line}");
    }
    println!("fim: {:?} {:?}", finish.status, finish.error_code);
    (finish, events)
}

/// A API de verdade nas duas portas, guardando o que cada chamada manda
struct Recording<'a> {
    api: Intermediary<'a>,
    sent: &'a Mutex<Vec<String>>,
}

impl Recording<'_> {
    fn record(&self, what: &str, payload: impl serde::Serialize) {
        let json = serde_json::to_string(&payload).unwrap();
        self.sent.lock().push(format!("{what}: {json}"));
    }
}

#[async_trait]
impl ModelBackend for Recording<'_> {
    async fn generate(
        &self,
        turn_id: &str,
        request: &AttemptRequest,
        cancel: &CancelFlag,
        progress: &(dyn Fn(AttemptProgress) + Send + Sync),
    ) -> Result<Generation> {
        self.record("tentativa", request);
        self.api.generate(turn_id, request, cancel, progress).await
    }
}

#[async_trait]
impl TurnJournal for Recording<'_> {
    async fn conversation(&self, conversation_id: &str) -> Result<AssistantConversation> {
        self.api.conversation(conversation_id).await
    }

    async fn open_turn(
        &self,
        conversation_id: &str,
        question: &str,
        language: Detected,
        client_turn_id: &str,
    ) -> Result<AssistantTurnSummary> {
        self.record("turno", (question, format!("{:?}", language.language)));
        self.api.open_turn(conversation_id, question, language, client_turn_id).await
    }

    async fn record_attempt(&self, turn_id: &str, request: &AttemptRequest, generation: &Generation) -> Result<()> {
        self.record("gravar tentativa", request);
        self.api.record_attempt(turn_id, request, generation).await
    }

    async fn report_outcome(&self, attempt_id: &str, outcome: &AttemptOutcome) -> Result<()> {
        self.record("outcome", outcome);
        self.api.report_outcome(attempt_id, outcome).await
    }

    async fn finish_turn(&self, turn_id: &str, status: AssistantTurnStatus) -> Result<()> {
        self.record("fim", status);
        self.api.finish_turn(turn_id, status).await
    }
}

/// A API guardou a última tentativa do turno com a execução bem-sucedida
pub(super) async fn assert_saved(participant: &Participant, turn_index: usize) {
    let saved = participant
        .client
        .conversation(&participant.token, &participant.conversation_id)
        .await
        .unwrap();
    let attempt = saved.turns[turn_index].attempts.last().unwrap();
    assert!(
        attempt.outcome.as_ref().is_some_and(|outcome| outcome.execution.ok == Some(true)),
        "{attempt:?}"
    );
}

#[tokio::test]
#[ignore]
async fn e2e_real_turn_against_the_assistant_api() {
    let Some(participant) = participant(DatabaseType::Sqlite).await else {
        return;
    };

    // Banco SQLite descartável
    let path = std::env::temp_dir().join("octapus_db_assistant_e2e.db");
    let _ = std::fs::remove_file(&path);
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute_batch(
            "CREATE TABLE customers (id INTEGER PRIMARY KEY, name TEXT NOT NULL, city TEXT);
             CREATE TABLE orders (id INTEGER PRIMARY KEY, customer_id INTEGER NOT NULL REFERENCES customers(id),
                                  total REAL NOT NULL, created_at TEXT NOT NULL);
             INSERT INTO customers (name, city) VALUES ('Ana', 'Porto Alegre'), ('Bia', 'Curitiba');
             INSERT INTO orders (customer_id, total, created_at) VALUES
               (1, 120.5, '2026-10-01'), (1, 80, '2026-10-05'), (2, 42, '2026-10-07');",
        )
        .unwrap();
    let server = server(DatabaseType::Sqlite, 0, Some(path.to_string_lossy().into_owned()));
    let (catalogs, entry, dir) = open_catalog(&server, "main").await;
    let db = AdapterDatabase {
        adapter: Arc::new(SqliteAdapter::new(&server, "main").unwrap()),
        db_type: DatabaseType::Sqlite,
    };

    let (finish, events) = ask(
        &participant,
        &db,
        Structure::Catalog(&catalogs, &entry),
        "Qual o total gasto por cada cliente?",
    )
    .await;
    assert_eq!(finish.status, AssistantTurnStatus::Answered, "{finish:?}");
    assert!(events.iter().any(|line| line.starts_with("result: 2 linha")));
    assert_multiline(&finish);
    assert_saved(&participant, 0).await;

    drop(entry);
    let _ = std::fs::remove_dir_all(dir);
    let _ = std::fs::remove_file(path);
}

/// A tabela citada entra no contexto mesmo fora do ranking (o banco tem
/// ruído demais para caber), e a menção inexistente vira aviso.
#[tokio::test]
#[ignore]
async fn e2e_real_turn_with_mentions() {
    let Some(participant) = participant(DatabaseType::Sqlite).await else {
        return;
    };

    let path = std::env::temp_dir().join("octapus_db_assistant_mentions_e2e.db");
    let _ = std::fs::remove_file(&path);
    let noise: String = (0..120)
        .map(|i| format!("CREATE TABLE item_log_{i:03} (id INTEGER PRIMARY KEY, item TEXT);"))
        .collect();
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute_batch(&format!(
            "{noise}
             CREATE TABLE zz_refunds (id INTEGER PRIMARY KEY, amount REAL NOT NULL, reason TEXT);
             INSERT INTO zz_refunds (amount, reason) VALUES (10, 'a'), (25.5, 'b'), (4.5, 'c');"
        ))
        .unwrap();
    let server = server(DatabaseType::Sqlite, 0, Some(path.to_string_lossy().into_owned()));
    let (catalogs, entry, dir) = open_catalog(&server, "main").await;
    let db = AdapterDatabase {
        adapter: Arc::new(SqliteAdapter::new(&server, "main").unwrap()),
        db_type: DatabaseType::Sqlite,
    };

    let mention = |table: &str| AssistantMention {
        schema: None,
        table: Some(table.into()),
    };
    let (finish, events) = ask_with(
        &participant,
        &db,
        Structure::Catalog(&catalogs, &entry),
        "Qual a soma dos valores dos itens em @zz_refunds? Ignore @ghost.",
        &[mention("zz_refunds"), mention("ghost")],
    )
    .await;
    assert_eq!(finish.status, AssistantTurnStatus::Answered, "{finish:?}");
    let context = events.iter().find(|line| line.starts_with("Context")).unwrap();
    assert!(context.contains("missing_mentions: [AssistantMention { schema: None, table: Some(\"ghost\") }]"), "{context}");
    assert!(events.iter().any(|line| line.starts_with("result: 1 linha")), "{events:?}");
    assert_saved(&participant, 0).await;

    drop(entry);
    let _ = std::fs::remove_dir_all(dir);
    let _ = std::fs::remove_file(path);
}

/// Contrato 1.5.0: dúvida conceitual e pergunta fora do tema viram
/// resposta em texto (nada executa); num banco grande, a tabela que só
/// aparece pelo nome leva o modelo a pedir a estrutura antes da consulta.
#[tokio::test]
#[ignore]
async fn e2e_real_answers_and_structure_request() {
    let Some(participant) = participant(DatabaseType::Sqlite).await else {
        return;
    };

    // Ruído que o ranking escolhe antes (nome) e a tabela da pergunta,
    // que só casa pelo sentido: entra só na linha "Other tables"
    let path = std::env::temp_dir().join("octapus_db_assistant_structure_e2e.db");
    let _ = std::fs::remove_file(&path);
    let noise: String = (0..200)
        .map(|i| format!("CREATE TABLE t_{i:04} (id INTEGER PRIMARY KEY, code TEXT);"))
        .collect();
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute_batch(&format!(
            "{noise}
             CREATE TABLE zz_refund_ledger (id INTEGER PRIMARY KEY, amount_cents INTEGER NOT NULL,
                                            reason TEXT, refunded_at TEXT NOT NULL);
             INSERT INTO zz_refund_ledger (amount_cents, reason, refunded_at) VALUES
               (1000, 'a', '2026-10-01'), (2550, 'b', '2026-10-02'), (450, 'c', '2026-10-03');"
        ))
        .unwrap();
    let server = server(DatabaseType::Sqlite, 0, Some(path.to_string_lossy().into_owned()));
    let (catalogs, entry, dir) = open_catalog(&server, "main").await;
    let db = AdapterDatabase {
        adapter: Arc::new(SqliteAdapter::new(&server, "main").unwrap()),
        db_type: DatabaseType::Sqlite,
    };

    // 1. Dúvida conceitual: texto, sem consulta
    let (finish, events) = ask(
        &participant,
        &db,
        Structure::Catalog(&catalogs, &entry),
        "O que é um índice composto e quando vale a pena criar um?",
    )
    .await;
    assert_eq!(finish.status, AssistantTurnStatus::Replied, "{finish:?}");
    let output = finish.output.as_ref().unwrap();
    assert!(output.query.is_none() && !output.explanation.trim().is_empty(), "{output:?}");
    assert!(!events.iter().any(|line| line.starts_with("result:") || line.starts_with("Plan")));
    println!("resposta: {}", output.explanation);

    // 2. Fora do tema: recusa curta, também em texto
    let (finish, _) = ask(
        &participant,
        &db,
        Structure::Catalog(&catalogs, &entry),
        "Qual é a capital da França e qual o melhor horário para visitar a Torre Eiffel?",
    )
    .await;
    assert_eq!(finish.status, AssistantTurnStatus::Replied, "{finish:?}");
    let refusal = &finish.output.as_ref().unwrap().explanation;
    assert!(!refusal.contains("Paris"), "{refusal}");
    println!("recusa: {refusal}");

    // 3. Banco grande: pede a estrutura e depois consulta
    let (finish, events) = ask(
        &participant,
        &db,
        Structure::Catalog(&catalogs, &entry),
        "Qual a soma dos valores dos reembolsos registrados?",
    )
    .await;
    assert_eq!(finish.status, AssistantTurnStatus::Answered, "{finish:?}");
    let lookup = events.iter().find(|line| line.starts_with("StructureLookup")).expect("sem pedido de estrutura");
    assert!(lookup.contains("found: 1") || lookup.contains("found: 2"), "{lookup}");
    assert!(events.iter().any(|line| line.starts_with("result: 1 linha")), "{events:?}");
    assert_saved(&participant, 2).await;

    drop(entry);
    let _ = std::fs::remove_dir_all(dir);
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
#[ignore = "requires the assistant API and local mongodb on localhost:27017 (no auth)"]
async fn e2e_real_mongo_turn_against_the_assistant_api() {
    use mongodb::bson::{doc, Document};

    const DATABASE: &str = "octapus_e2e_assistant_turn";
    let Some(participant) = participant(DatabaseType::Mongodb).await else {
        return;
    };

    // Banco Mongo descartável: pedidos com o cliente embutido
    let mongo = mongodb::Client::with_uri_str("mongodb://localhost:27017").await.unwrap();
    let database = mongo.database(DATABASE);
    database.drop().await.unwrap();
    database
        .collection::<Document>("orders")
        .insert_many([
            doc! { "customer": { "name": "Ana", "city": "Porto Alegre" }, "total": 120.5, "status": "paid" },
            doc! { "customer": { "name": "Ana", "city": "Porto Alegre" }, "total": 80.0, "status": "paid" },
            doc! { "customer": { "name": "Bia", "city": "Curitiba" }, "total": 42.0, "status": "paid" },
            doc! { "customer": { "name": "Caio", "city": "Curitiba" }, "total": 15.0, "status": "cancelled" },
        ])
        .await
        .unwrap();
    database
        .collection::<Document>("products")
        .insert_many([doc! { "sku": "A-1", "price": 10.0 }, doc! { "sku": "B-2", "price": 25.0 }])
        .await
        .unwrap();

    let server = server(DatabaseType::Mongodb, 27017, None);
    let (catalogs, entry, dir) = open_catalog(&server, DATABASE).await;
    let db = AdapterDatabase {
        adapter: Arc::new(MongoAdapter::new(&server, DATABASE).await.unwrap()),
        db_type: DatabaseType::Mongodb,
    };

    let (finish, events) = ask(
        &participant,
        &db,
        Structure::Catalog(&catalogs, &entry),
        "Qual o total vendido por cidade, só nos pedidos pagos?",
    )
    .await;
    assert_eq!(finish.status, AssistantTurnStatus::Answered, "{finish:?}");
    assert!(events.iter().any(|line| line.starts_with("result: 2 linha")));
    assert_saved(&participant, 0).await;

    drop(entry);
    let _ = std::fs::remove_dir_all(dir);
    database.drop().await.unwrap();
}

#[tokio::test]
#[ignore = "requires the assistant API and local redis on localhost:6380 (no auth)"]
async fn e2e_real_redis_turns_against_the_assistant_api() {
    let Some(participant) = participant(DatabaseType::Redis).await else {
        return;
    };

    // Redis descartável (db 2): usuários em hash, carrinhos em lista e um ranking
    let adapter = RedisAdapter::new(&server(DatabaseType::Redis, 6380, None), "2").unwrap();
    adapter.execute_statement("FLUSHDB").await.unwrap();
    let users = [("Ana", "Porto Alegre"), ("Bia", "Curitiba"), ("Caio", "Recife"), ("Duda", "Natal"), ("Edu", "Belém")];
    for (i, (name, city)) in users.iter().enumerate() {
        adapter
            .execute_statement(&format!("HSET user:{} name \"{name}\" city \"{city}\"", 1001 + i))
            .await
            .unwrap();
    }
    adapter.execute_statement("RPUSH user:1001:cart A-1 B-2").await.unwrap();
    adapter.execute_statement("ZADD leaderboard 10 user:1001 7 user:1002 3 user:1003").await.unwrap();
    let db = AdapterDatabase {
        adapter: Arc::new(adapter),
        db_type: DatabaseType::Redis,
    };

    // Contagem pelo SCAN
    let (finish, events) = ask(&participant, &db, Structure::Keys, "Quantos usuários existem?").await;
    assert_eq!(finish.status, AssistantTurnStatus::Answered, "{finish:?}");
    assert!(events.iter().any(|line| line.starts_with("result: ")));
    assert_saved(&participant, 0).await;

    // Seguimento da conversa: foreach com HGET em cada usuário
    let (finish, events) = ask(&participant, &db, Structure::Keys, "E qual é o nome e a cidade de cada um?").await;
    assert_eq!(finish.status, AssistantTurnStatus::Answered, "{finish:?}");
    assert!(events.iter().any(|line| line.starts_with("result: 5 linha")));
    assert_saved(&participant, 1).await;

    // Dúvida conceitual (redis-v2): texto, sem plano
    assert_replied(
        ask(
            &participant,
            &db,
            Structure::Keys,
            "Qual a diferença entre guardar um usuário num hash e numa string com JSON?",
        )
        .await,
    );

    db.adapter.execute_statement("FLUSHDB").await.unwrap();
}

/// Resposta em texto: nada executa e há explicação
fn assert_replied((finish, events): (AssistantTurnFinish, Vec<String>)) {
    assert_eq!(finish.status, AssistantTurnStatus::Replied, "{finish:?}");
    let output = finish.output.as_ref().unwrap();
    assert!(output.query.is_none() && !output.explanation.trim().is_empty(), "{output:?}");
    assert!(!events.iter().any(|line| line.starts_with("result:") || line.starts_with("Plan")));
    println!("resposta: {}", output.explanation);
}

/// Pediu a estrutura do que não veio no recorte e depois consultou
fn assert_looked_up_then_answered((finish, events): (AssistantTurnFinish, Vec<String>)) {
    assert_eq!(finish.status, AssistantTurnStatus::Answered, "{finish:?}");
    let lookup = events
        .iter()
        .find(|line| line.starts_with("StructureLookup"))
        .expect("sem pedido de estrutura");
    assert!(!lookup.contains("found: 0"), "{lookup}");
    assert!(events.iter().any(|line| line.starts_with("result: 1 linha")), "{events:?}");
}

/// Prompts v2 no Postgres: dúvida conceitual em texto e, num schema de
/// 200 tabelas, o pedido de estrutura antes da consulta.
#[tokio::test]
#[ignore = "requires the assistant API and Postgres on localhost:5432 (postgres/1234)"]
async fn e2e_real_postgres_answers_and_structure_request() {
    use crate::adapters::postgres::PostgresAdapter;

    const DATABASE: &str = "octapus_e2e_assistant_v2";
    let Some(participant) = participant(DatabaseType::Postgres).await else {
        return;
    };
    let pg_server = Server {
        username: "postgres".into(),
        password: "1234".into(),
        ..server(DatabaseType::Postgres, 5432, None)
    };
    let admin = PostgresAdapter::new(&pg_server, "postgres").unwrap();
    admin
        .execute_statement(&format!("DROP DATABASE IF EXISTS {DATABASE} WITH (FORCE)"))
        .await
        .unwrap();
    admin.execute_statement(&format!("CREATE DATABASE {DATABASE}")).await.unwrap();
    // O adapter executa um comando por vez
    let scratch = PostgresAdapter::new(&pg_server, DATABASE).unwrap();
    let noise = (0..200).map(|i| format!("CREATE TABLE t_{i:04} (id serial PRIMARY KEY, code text)"));
    let ledger = [
        "CREATE TABLE zz_refund_ledger (id serial PRIMARY KEY, amount_cents int NOT NULL,
                                        reason text, refunded_at date NOT NULL)"
            .to_string(),
        "INSERT INTO zz_refund_ledger (amount_cents, reason, refunded_at) VALUES
           (1000, 'a', '2026-10-01'), (2550, 'b', '2026-10-02'), (450, 'c', '2026-10-03')"
            .to_string(),
    ];
    for statement in noise.chain(ledger) {
        scratch.execute_statement(&statement).await.unwrap();
    }

    let (catalogs, entry, dir) = open_catalog(&pg_server, DATABASE).await;
    let db = AdapterDatabase {
        adapter: Arc::new(scratch),
        db_type: DatabaseType::Postgres,
    };
    let structure = || Structure::Catalog(&catalogs, &entry);

    assert_replied(
        ask(
            &participant,
            &db,
            structure(),
            "Qual a diferença entre um índice B-tree e um GIN no Postgres?",
        )
        .await,
    );
    assert_looked_up_then_answered(
        ask(&participant, &db, structure(), "Qual a soma dos valores dos reembolsos registrados?").await,
    );
    assert_saved(&participant, 1).await;

    drop(db);
    drop(entry);
    let _ = std::fs::remove_dir_all(dir);
    admin
        .execute_statement(&format!("DROP DATABASE IF EXISTS {DATABASE} WITH (FORCE)"))
        .await
        .unwrap();
}

/// Prompts v4: o SQL vem em várias linhas, uma cláusula por linha
fn assert_multiline(finish: &AssistantTurnFinish) {
    let query = finish
        .output
        .as_ref()
        .and_then(|output| output.query.as_deref())
        .unwrap_or_default();
    println!("consulta:\n{query}");
    assert!(query.lines().count() >= 3, "consulta numa linha só: {query:?}");
    assert!(
        query.lines().any(|line| line.trim_start().starts_with("FROM")),
        "FROM sem linha própria: {query:?}"
    );
}

/// Prompts v4 no Postgres: a consulta com junção e agrupamento chega
/// formatada, em várias linhas, e roda.
#[tokio::test]
#[ignore = "requires the assistant API and Postgres on localhost:5432 (postgres/1234)"]
async fn e2e_real_postgres_query_in_several_lines() {
    use crate::adapters::postgres::PostgresAdapter;

    const DATABASE: &str = "octapus_e2e_assistant_v4";
    let Some(participant) = participant(DatabaseType::Postgres).await else {
        return;
    };
    let pg_server = Server {
        username: "postgres".into(),
        password: "1234".into(),
        ..server(DatabaseType::Postgres, 5432, None)
    };
    let admin = PostgresAdapter::new(&pg_server, "postgres").unwrap();
    admin
        .execute_statement(&format!("DROP DATABASE IF EXISTS {DATABASE} WITH (FORCE)"))
        .await
        .unwrap();
    admin
        .execute_statement(&format!("CREATE DATABASE {DATABASE}"))
        .await
        .unwrap();
    let scratch = PostgresAdapter::new(&pg_server, DATABASE).unwrap();
    for statement in [
        "CREATE TABLE customers (id serial PRIMARY KEY, name text NOT NULL, city text)",
        "CREATE TABLE orders (id serial PRIMARY KEY, customer_id int NOT NULL REFERENCES customers(id),
                              total numeric(10,2) NOT NULL, created_at timestamptz NOT NULL DEFAULT now())",
        "INSERT INTO customers (name, city) VALUES ('Ana', 'Porto Alegre'), ('Bia', 'Curitiba')",
        "INSERT INTO orders (customer_id, total) VALUES (1, 120.5), (1, 80), (2, 42)",
    ] {
        scratch.execute_statement(statement).await.unwrap();
    }

    let (catalogs, entry, dir) = open_catalog(&pg_server, DATABASE).await;
    let db = AdapterDatabase {
        adapter: Arc::new(scratch),
        db_type: DatabaseType::Postgres,
    };
    let (finish, events) = ask(
        &participant,
        &db,
        Structure::Catalog(&catalogs, &entry),
        "Qual o total gasto e o número de pedidos de cada cliente, com a cidade, do maior para o menor?",
    )
    .await;
    assert_eq!(finish.status, AssistantTurnStatus::Answered, "{finish:?}");
    assert!(
        events.iter().any(|line| line.starts_with("result: 2 linha")),
        "{events:?}"
    );
    assert_multiline(&finish);

    drop(db);
    drop(entry);
    let _ = std::fs::remove_dir_all(dir);
    admin
        .execute_statement(&format!("DROP DATABASE IF EXISTS {DATABASE} WITH (FORCE)"))
        .await
        .unwrap();
}

/// Prompts v5 no Postgres: a coluna que a pergunta pede está numa de 60
/// tabelas de nome opaco, fora das que vão com colunas — pedir uma a uma
/// passa do teto de nomes, então o modelo acha pela busca `columns:`. O
/// comentário do schema vai no cabeçalho.
#[tokio::test]
#[ignore = "requires the assistant API and Postgres on localhost:5432 (postgres/1234)"]
async fn e2e_real_postgres_column_search() {
    use crate::adapters::postgres::PostgresAdapter;

    const DATABASE: &str = "octapus_e2e_assistant_v5";
    let Some(participant) = participant(DatabaseType::Postgres).await else {
        return;
    };
    let pg_server = Server {
        username: "postgres".into(),
        password: "1234".into(),
        ..server(DatabaseType::Postgres, 5432, None)
    };
    let admin = PostgresAdapter::new(&pg_server, "postgres").unwrap();
    admin
        .execute_statement(&format!("DROP DATABASE IF EXISTS {DATABASE} WITH (FORCE)"))
        .await
        .unwrap();
    admin
        .execute_statement(&format!("CREATE DATABASE {DATABASE}"))
        .await
        .unwrap();
    let scratch = PostgresAdapter::new(&pg_server, DATABASE).unwrap();
    let mut statements: Vec<String> = (0..60)
        .filter(|i| *i != 47)
        .map(|i| format!("CREATE TABLE t_{i:03} (id serial PRIMARY KEY, valor int, criado_em date)"))
        .collect();
    statements.extend(
        [
            "COMMENT ON SCHEMA public IS 'Cadastro de pessoas e lançamentos'",
            "CREATE TABLE t_047 (id serial PRIMARY KEY, nome_completo text NOT NULL, documento_cpf text)",
            "INSERT INTO t_047 (nome_completo, documento_cpf) VALUES ('Ana Souza', '111'), ('Bia Lima', '222')",
        ]
        .map(String::from),
    );
    for statement in statements {
        scratch.execute_statement(&statement).await.unwrap();
    }

    let (catalogs, entry, dir) = open_catalog(&pg_server, DATABASE).await;
    let db = AdapterDatabase {
        adapter: Arc::new(scratch),
        db_type: DatabaseType::Postgres,
    };
    let (finish, events) = ask(
        &participant,
        &db,
        Structure::Catalog(&catalogs, &entry),
        "Qual o CPF da Ana Souza?",
    )
    .await;
    assert_eq!(finish.status, AssistantTurnStatus::Answered, "{finish:?}");
    assert!(events.iter().any(|line| line.starts_with("result: 1 linha")), "{events:?}");
    let sent = participant.sent.lock().join("\n");
    assert!(sent.contains("-- Columns matching"), "o modelo não buscou a coluna: {events:?}");
    assert!(sent.contains("-- Schema \\\"public\\\": «Cadastro de pessoas e lançamentos»"), "{sent}");

    drop(db);
    drop(entry);
    let _ = std::fs::remove_dir_all(dir);
    admin
        .execute_statement(&format!("DROP DATABASE IF EXISTS {DATABASE} WITH (FORCE)"))
        .await
        .unwrap();
}

/// Prompts v4 no Mongo: o `status` guarda códigos em maiúsculas. Com o
/// interruptor desligado, o modelo não vê valor nenhum; ligado, vê os
/// valores de exemplo e filtra com o certo. Mongo sem auth em
/// `OCTAPUS_E2E_MONGO_PORT` (padrão 27017).
#[tokio::test]
#[ignore = "requires the assistant API and local mongodb without auth (OCTAPUS_E2E_MONGO_PORT)"]
async fn e2e_real_mongo_sample_values() {
    use mongodb::bson::{doc, Document};

    const DATABASE: &str = "octapus_e2e_assistant_values";
    let Some(participant) = participant(DatabaseType::Mongodb).await else {
        return;
    };
    let port: u16 = std::env::var("OCTAPUS_E2E_MONGO_PORT")
        .ok()
        .and_then(|port| port.parse().ok())
        .unwrap_or(27017);
    let mongo = mongodb::Client::with_uri_str(format!("mongodb://localhost:{port}"))
        .await
        .unwrap();
    let database = mongo.database(DATABASE);
    database.drop().await.unwrap();
    database
        .create_collection("chamados")
        .validator(doc! { "$jsonSchema": {
            "description": "Chamados do suporte",
            "properties": { "situacao": { "description": "Situação do pagamento do chamado" } },
        } })
        .await
        .unwrap();
    let tickets: Vec<Document> = (0..40)
        .map(|i| {
            let situacao = match i % 4 {
                0 | 1 => "PG_CONFIRMADO",
                2 => "AGUARDANDO_PG",
                _ => "ESTORNADO",
            };
            doc! { "situacao": situacao, "valor": 10.0 + i as f64 }
        })
        .collect();
    database.collection::<Document>("chamados").insert_many(tickets).await.unwrap();

    let server = server(DatabaseType::Mongodb, port, None);
    let (catalogs, entry, dir) = open_catalog(&server, DATABASE).await;
    let db = AdapterDatabase {
        adapter: Arc::new(MongoAdapter::new(&server, DATABASE).await.unwrap()),
        db_type: DatabaseType::Mongodb,
    };
    let question = "Qual o valor total dos chamados já pagos?";

    let (off, _) = ask_full(&participant, &db, Structure::Catalog(&catalogs, &entry), question, &[], false).await;
    println!("desligado: {:?} {:?}", off.status, off.output.as_ref().map(|output| &output.query));
    let sent_off = participant.sent.lock().join("\n");
    // (o `values:` aparece no próprio prompt, como exemplo; os valores do banco, não)
    for value in ["PG_CONFIRMADO", "AGUARDANDO_PG", "ESTORNADO"] {
        assert!(!sent_off.contains(value), "{value}: {sent_off}");
    }
    assert!(sent_off.contains("Situação do pagamento do chamado"), "{sent_off}");

    let (on, events) = ask_full(&participant, &db, Structure::Catalog(&catalogs, &entry), question, &[], true).await;
    assert_eq!(on.status, AssistantTurnStatus::Answered, "{on:?}");
    let query = on.output.as_ref().and_then(|output| output.query.clone()).unwrap_or_default();
    assert!(query.contains("PG_CONFIRMADO"), "{query}");
    assert!(events.iter().any(|line| line.starts_with("result: 1 linha")), "{events:?}");
    let sent = participant.sent.lock().join("\n");
    assert!(sent.contains("values: \\\"PG_CONFIRMADO\\\" 50%"), "{sent}");

    drop(entry);
    let _ = std::fs::remove_dir_all(dir);
    database.drop().await.unwrap();
}

/// Prompts v3 no Postgres: o modelo pede índices e a definição de uma
/// função pelos prefixos, e os literais dela (canários) não chegam à API.
#[tokio::test]
#[ignore = "requires the assistant API and Postgres on localhost:5432 (postgres/1234)"]
async fn e2e_real_postgres_object_requests_without_literals() {
    use crate::adapters::postgres::PostgresAdapter;

    const DATABASE: &str = "octapus_e2e_assistant_v3";
    let Some(participant) = participant(DatabaseType::Postgres).await else {
        return;
    };
    let pg_server = Server {
        username: "postgres".into(),
        password: "1234".into(),
        ..server(DatabaseType::Postgres, 5432, None)
    };
    let admin = PostgresAdapter::new(&pg_server, "postgres").unwrap();
    admin
        .execute_statement(&format!("DROP DATABASE IF EXISTS {DATABASE} WITH (FORCE)"))
        .await
        .unwrap();
    admin
        .execute_statement(&format!("CREATE DATABASE {DATABASE}"))
        .await
        .unwrap();
    let scratch = PostgresAdapter::new(&pg_server, DATABASE).unwrap();
    let noise = (0..200).map(|i| format!("CREATE TABLE t_{i:04} (id serial PRIMARY KEY, code text)"));
    let objects = [
        "CREATE TABLE zz_orders (id serial PRIMARY KEY, status text NOT NULL, total_cents int NOT NULL)",
        "CREATE INDEX zz_orders_status_idx ON zz_orders (status) WHERE status <> 'CANARY_void'",
        "CREATE FUNCTION zz_order_total(p_order int) RETURNS int LANGUAGE plpgsql AS $$
           BEGIN
             IF p_order = 424242 THEN RAISE EXCEPTION 'CANARY_blocked'; END IF;
             RETURN (SELECT total_cents FROM zz_orders WHERE id = p_order AND status <> 'CANARY_void');
           END $$",
    ]
    .map(String::from);
    for statement in noise.chain(objects) {
        scratch.execute_statement(&statement).await.unwrap();
    }

    let (catalogs, entry, dir) = open_catalog(&pg_server, DATABASE).await;
    let db = AdapterDatabase {
        adapter: Arc::new(scratch),
        db_type: DatabaseType::Postgres,
    };
    let structure = || Structure::Catalog(&catalogs, &entry);
    let looked_up = |events: &[String]| events.iter().any(|line| line.starts_with("StructureLookup"));

    let (finish, events) = ask(
        &participant,
        &db,
        structure(),
        "Quais índices a tabela zz_orders tem? Algum deles é parcial?",
    )
    .await;
    println!("fim 1: {:?}", finish.status);
    assert!(looked_up(&events), "{events:?}");

    let (finish, events) = ask(
        &participant,
        &db,
        structure(),
        "O que a função zz_order_total faz, passo a passo?",
    )
    .await;
    println!("fim 2: {:?}", finish.status);
    assert!(looked_up(&events), "{events:?}");

    let sent = participant.sent.lock().join("\n");
    assert!(
        sent.contains("indexes:") || sent.contains("definition:"),
        "sem prefixo pedido"
    );
    assert!(
        sent.contains("definition:") || sent.contains("functions:"),
        "sem pedido da função"
    );
    assert!(
        !sent.contains("CANARY") && !sent.contains("424242"),
        "literal vazou para a API"
    );

    drop(db);
    drop(entry);
    let _ = std::fs::remove_dir_all(dir);
    admin
        .execute_statement(&format!("DROP DATABASE IF EXISTS {DATABASE} WITH (FORCE)"))
        .await
        .unwrap();
}

/// Prompts v2 no Mongo: dúvida conceitual em texto e, com mais coleções
/// que as 12 do contexto, o pedido de estrutura antes da consulta.
#[tokio::test]
#[ignore = "requires the assistant API and local mongodb on localhost:27017 (no auth)"]
async fn e2e_real_mongo_answers_and_structure_request() {
    use mongodb::bson::{doc, Document};

    const DATABASE: &str = "octapus_e2e_assistant_v2";
    let Some(participant) = participant(DatabaseType::Mongodb).await else {
        return;
    };
    let mongo = mongodb::Client::with_uri_str("mongodb://localhost:27017").await.unwrap();
    let database = mongo.database(DATABASE);
    database.drop().await.unwrap();
    for i in 0..20 {
        database
            .collection::<Document>(&format!("c_{i:02}"))
            .insert_one(doc! { "code": "x" })
            .await
            .unwrap();
    }
    database
        .collection::<Document>("zz_refund_ledger")
        .insert_many([
            doc! { "amountCents": 1000, "reason": "a" },
            doc! { "amountCents": 2550, "reason": "b" },
            doc! { "amountCents": 450, "reason": "c" },
        ])
        .await
        .unwrap();

    let server = server(DatabaseType::Mongodb, 27017, None);
    let (catalogs, entry, dir) = open_catalog(&server, DATABASE).await;
    let db = AdapterDatabase {
        adapter: Arc::new(MongoAdapter::new(&server, DATABASE).await.unwrap()),
        db_type: DatabaseType::Mongodb,
    };
    let structure = || Structure::Catalog(&catalogs, &entry);

    assert_replied(
        ask(
            &participant,
            &db,
            structure(),
            "Quando vale mais a pena embutir documentos do que referenciar outra coleção?",
        )
        .await,
    );
    assert_looked_up_then_answered(
        ask(&participant, &db, structure(), "Qual a soma dos valores dos reembolsos registrados?").await,
    );
    assert_saved(&participant, 1).await;

    drop(entry);
    let _ = std::fs::remove_dir_all(dir);
    database.drop().await.unwrap();
}
