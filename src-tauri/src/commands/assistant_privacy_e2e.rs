//! Prova de privacidade do assistente (B10, critérios 7 e 8 do épico), com
//! turnos de verdade contra a API local.
//!
//! Cada teste semeia um banco descartável com **canários**: strings únicas
//! desta execução nos valores, nos campos de texto livre (observações,
//! comentários), nas chaves do Redis com cara de registro e nas chaves de mapa
//! do Mongo. Roda os fluxos do assistente — contexto da estrutura, erro do
//! banco que cita um valor e a correção, pergunta de seguimento (histórico),
//! execução manual com paginação e avaliação com comentário — e depois procura
//! cada canário em quatro lugares:
//!
//! 1. no que o app mandou à API (a `Recording` do `e2e` grava cada chamada);
//! 2. no log do app, capturado com os mesmos níveis do arquivo de log
//!    (`logging.rs`): é o que o `tauri-plugin-log` gravaria em disco;
//! 3. em todas as tabelas do banco da API, numa transação só de leitura;
//! 4. no log do container da API (`docker logs`).
//!
//! Nenhum pode aparecer. E, para a prova não ser vazia, cada teste confere que
//! o canário chegou à tela (o erro como o banco deu, as linhas do resultado):
//! ele passou pelo app e parou ali.
//!
//! O de Postgres prova também o critério 8: o histórico volta noutra "máquina"
//! — um cliente novo, sem nada da sessão anterior, só com o convite.
//!
//! ```text
//! OCTAPUS_E2E_ASSISTANT_URL=http://localhost:8787 OCTAPUS_E2E_INVITE=<convite> \
//! OCTAPUS_E2E_API_DATABASE_URL=postgres://…@localhost:54330/octapus_assistant \
//! OCTAPUS_E2E_API_CONTAINER=octapus-assistant-api-api-1 \
//!   cargo test --lib commands::assistant::privacy_e2e -- --ignored --nocapture
//! ```
//!
//! Bancos: Postgres em localhost:5432 (postgres/1234, cria e apaga
//! `octapus_e2e_b10_privacy`), Mongo em localhost:27017 e Redis em
//! localhost:6380 (db 3), os dois sem auth.

use std::sync::{Arc, Mutex as StdMutex, Once};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use log::LevelFilter;

use super::e2e::{ask, assert_saved, client_turn_id, open_catalog, participant, server, Participant, Structure};
use super::*;
use crate::adapters::mongo::MongoAdapter;
use crate::adapters::postgres::PostgresAdapter;
use crate::adapters::redisdb::RedisAdapter;
use crate::adapters::sqlite::SqliteAdapter;
use crate::logging::{app_level, APP_TARGET, DEPENDENCIES_LEVEL};
use crate::models::{FeedbackRating, FeedbackReason, Server};

/// O banco descartável de cada paradigma.
const SCRATCH: &str = "octapus_e2e_b10_privacy";

// ── Canários ────────────────────────────────────────────────────────────────

/// Strings únicas desta execução, em formatos que cobrem a generalização das
/// chaves do Redis e do Mongo: um nome, um número, um hex e um UUID.
struct Canaries {
    /// Só letras fora do hex: tem cara de nome, nunca de id
    word: String,
    /// 13 dígitos (um id numérico, um documento)
    number: String,
    /// 32 hex (um token de sessão)
    hex: String,
    uuid: String,
}

impl Canaries {
    fn new() -> Self {
        const LETTERS: &[u8] = b"ghjkmnpqrstvwxyz";
        let word: String = (0..10)
            .map(|_| LETTERS[rand::random::<usize>() % LETTERS.len()] as char)
            .collect();
        let number = format!("{}{:012}", 1 + rand::random::<u8>() % 9, rand::random::<u64>() % 1_000_000_000_000);
        let hex = format!("{:032x}", rand::random::<u128>());
        Self {
            word: format!("cnry{word}"),
            number,
            hex,
            uuid: client_turn_id(),
        }
    }

    /// O que não pode aparecer: cada canário e um pedaço dos de id (um
    /// vazamento parcial também conta).
    fn needles(&self) -> Vec<String> {
        vec![
            self.word.clone(),
            self.number.clone(),
            self.number[..10].to_string(),
            self.hex.clone(),
            self.hex[..12].to_string(),
            self.uuid.clone(),
            self.uuid[24..].to_string(),
        ]
    }

    /// Os canários encontrados no texto (sem diferenciar maiúsculas).
    fn found_in(&self, text: &str) -> Vec<String> {
        let text = text.to_lowercase();
        self.needles().into_iter().filter(|needle| text.contains(needle)).collect()
    }
}

// ── Log do app ──────────────────────────────────────────────────────────────

/// Tudo que o app logou desde que a captura começou.
static CAPTURED: StdMutex<Vec<String>> = StdMutex::new(Vec::new());

/// Um `log::Log` com o filtro do arquivo de log do app: o que passa aqui é o
/// que o `tauri-plugin-log` gravaria.
struct Capture;

impl log::Log for Capture {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        let level = if metadata.target().starts_with(APP_TARGET) {
            app_level()
        } else {
            DEPENDENCIES_LEVEL
        };
        metadata.level() <= level
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            let line = format!("{} {} {}", record.level(), record.target(), record.args());
            CAPTURED.lock().unwrap().push(line);
        }
    }

    fn flush(&self) {}
}

fn capture_logs() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        log::set_logger(&Capture).expect("nenhum outro logger nos testes");
        log::set_max_level(LevelFilter::Trace);
    });
}

// ── Onde procurar ───────────────────────────────────────────────────────────

/// Cada linha de todas as tabelas do banco da API, como texto, numa transação
/// só de leitura (o banco da API não é nosso para alterar).
async fn api_database_rows() -> Vec<(String, String)> {
    let url = std::env::var("OCTAPUS_E2E_API_DATABASE_URL")
        .expect("OCTAPUS_E2E_API_DATABASE_URL: o banco da API, para a varredura");
    let (mut client, connection) = tokio_postgres::connect(&url, tokio_postgres::NoTls).await.unwrap();
    tokio::spawn(connection);
    client
        .batch_execute("SET SESSION CHARACTERISTICS AS TRANSACTION READ ONLY")
        .await
        .unwrap();
    let tx = client.build_transaction().read_only(true).start().await.unwrap();

    let tables: Vec<String> = tx
        .query(
            "SELECT quote_ident(table_schema) || '.' || quote_ident(table_name)
               FROM information_schema.tables
              WHERE table_type = 'BASE TABLE'
                AND table_schema NOT IN ('pg_catalog', 'information_schema')
              ORDER BY 1",
            &[],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| row.get(0))
        .collect();
    assert!(tables.iter().any(|table| table == "public.attempts"), "{tables:?}");

    let mut rows = Vec::new();
    for table in tables {
        for row in tx.query(&format!("SELECT t::text FROM {table} t"), &[]).await.unwrap() {
            rows.push((table.clone(), row.get(0)));
        }
    }
    tx.rollback().await.unwrap();
    rows
}

/// O log do container da API desde `since` (segundos Unix), stdout e stderr.
fn api_container_logs(since: u64) -> String {
    let container = std::env::var("OCTAPUS_E2E_API_CONTAINER")
        .expect("OCTAPUS_E2E_API_CONTAINER: o container da API, para ler o log");
    let output = std::process::Command::new("docker")
        .args(["logs", "--since", &since.to_string(), &container])
        .output()
        .expect("docker no PATH");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8_lossy(&output.stdout).into_owned() + &String::from_utf8_lossy(&output.stderr)
}

fn now_unix() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

/// Procura os canários nos quatro lugares; nenhum pode aparecer (critério 7).
async fn assert_nothing_leaked(canaries: &Canaries, participant: &Participant, since: u64) {
    let mut leaks: Vec<String> = Vec::new();
    let mut check = |place: &str, text: &str| {
        let found = canaries.found_in(text);
        if !found.is_empty() {
            let excerpt: String = text.chars().take(400).collect();
            leaks.push(format!("{place}: {found:?} em {excerpt}"));
        }
    };

    // 1. O que o app mandou
    let sent = participant.sent.lock().clone();
    assert!(sent.iter().any(|line| line.starts_with("tentativa: ")), "nenhuma tentativa gravada");
    for line in &sent {
        check("enviado à API", line);
    }

    // 2. O log do app
    let logs = CAPTURED.lock().unwrap().clone();
    assert!(logs.iter().any(|line| line.contains("assistente: turno")), "o log do app não foi capturado");
    for line in &logs {
        check("log do app", line);
    }

    // 3. O banco da API: a conversa desta execução está lá, sem canário
    let rows = api_database_rows().await;
    assert!(
        rows.iter().any(|(_, row)| row.contains(&participant.conversation_id)),
        "a conversa {} não está no banco da API",
        participant.conversation_id
    );
    for (table, row) in &rows {
        check(&format!("banco da API ({table})"), row);
    }

    // 4. O log do container da API
    let api_logs = api_container_logs(since);
    assert!(
        api_logs.contains(&participant.conversation_id),
        "o log da API não tem a conversa {}",
        participant.conversation_id
    );
    for line in api_logs.lines() {
        check("log da API", line);
    }

    println!(
        "varredura: {} envio(s), {} linha(s) de log do app, {} linha(s) do banco da API, {} linha(s) de log da API",
        sent.len(),
        logs.len(),
        rows.len(),
        api_logs.lines().count()
    );
    assert!(leaks.is_empty(), "canário fora do app:\n{}", leaks.join("\n"));
}

// ── Fluxos ──────────────────────────────────────────────────────────────────

/// A execução manual, como no `assistant_execute`: o caminho só-leitura e, na
/// primeira página de uma tentativa, o outcome completado na API.
async fn execute_manually(
    participant: &Participant,
    db: &AdapterDatabase,
    query: &str,
    page: QueryOptions,
    attempt_id: &str,
    user_edited: bool,
) -> Result<QueryResult> {
    let op = ReadOnlyOp::new(query_language(db.db_type), query);
    let offset = page.offset;
    let started = Instant::now();
    let result = read_only::execute(db.adapter.as_ref(), db.db_type, &op, page).await;
    let duration = Some(started.elapsed().as_millis() as i64);

    if offset == 0 {
        let execution = ManualExecution {
            result: &result,
            query,
            duration,
            user_edited,
        };
        report_manual_execution(&participant.assistant, &participant.client, &participant.token, attempt_id, execution)
            .await;
        let outcome = participant.assistant.outcome(attempt_id);
        let json = serde_json::to_string(&outcome).unwrap();
        participant.sent.lock().push(format!("outcome manual: {json}"));
    }
    result
}

/// Alguma tentativa do turno falhou no banco e foi para a correção (C-11).
async fn corrected_attempts(participant: &Participant, turn_index: usize) -> usize {
    let saved = participant
        .client
        .conversation(&participant.token, &participant.conversation_id)
        .await
        .unwrap();
    saved.turns[turn_index]
        .attempts
        .iter()
        .filter(|attempt| {
            attempt.outcome.as_ref().is_some_and(|outcome| {
                outcome.execution.ok == Some(false) && outcome.execution.error_sanitized.is_some()
            })
        })
        .count()
}

/// Um cliente novo, sem nada desta sessão, resgata o mesmo convite e lê a
/// conversa: o histórico vem do servidor (C-15, critério 8).
async fn history_on_another_machine(participant: &Participant, questions: &[&str]) {
    let url = std::env::var("OCTAPUS_E2E_ASSISTANT_URL").unwrap();
    let invite = std::env::var("OCTAPUS_E2E_INVITE").unwrap();
    let client = ApiClient::new(&url).unwrap();
    let token = client.redeem(&invite, Some("octapus e2e (outra máquina)")).await.unwrap().token;
    assert_ne!(token, participant.token);

    let mut cursor: Option<String> = None;
    let summary = loop {
        let page = client.conversations(&token, cursor.as_deref(), CONVERSATIONS_PAGE).await.unwrap();
        if let Some(found) = page.items.into_iter().find(|item| item.id == participant.conversation_id) {
            break found;
        }
        cursor = Some(page.next_cursor.expect("a conversa está na lista do participante"));
    };
    assert_eq!(summary.paradigm, "postgres");

    let conversation = client.conversation(&token, &participant.conversation_id).await.unwrap();
    let saved: Vec<&str> = conversation.turns.iter().map(|turn| turn.summary.question.as_str()).collect();
    assert_eq!(saved, questions);
    for turn in &conversation.turns {
        let last = turn.attempts.last().expect("tentativas no histórico");
        assert!(last.output.as_ref().is_some_and(|output| output.query.is_some()), "{last:?}");
    }
    let feedback = conversation.turns[1].feedback.as_ref().expect("a avaliação voltou com a conversa");
    assert_eq!(feedback.rating, FeedbackRating::Down);
    assert!(feedback.comment.is_some());
}

// ── Testes ──────────────────────────────────────────────────────────────────

#[tokio::test]
#[ignore = "requires the assistant API, its database and container, and Postgres on localhost:5432"]
async fn e2e_privacy_postgres_canaries() {
    capture_logs();
    let since = now_unix().saturating_sub(5);
    let Some(participant) = participant(DatabaseType::Postgres).await else {
        return;
    };
    let canaries = Canaries::new();
    let Canaries { word, number, hex, uuid } = &canaries;

    // Banco descartável no Postgres dos e2e
    let admin_server = server(DatabaseType::Postgres, 5432, None);
    let pg_server = Server {
        username: "postgres".into(),
        password: "1234".into(),
        ..admin_server
    };
    let admin = pg_client("postgres").await;
    for statement in [format!("DROP DATABASE IF EXISTS {SCRATCH} WITH (FORCE)"), format!("CREATE DATABASE {SCRATCH}")] {
        admin.batch_execute(&statement).await.unwrap();
    }
    pg_client(SCRATCH)
        .await
        .batch_execute(&format!(
            "CREATE TABLE customers (id serial PRIMARY KEY, name text NOT NULL, email text NOT NULL,
                                     city text NOT NULL, notes text);
             COMMENT ON TABLE customers IS 'Clientes da loja';
             CREATE TABLE payments (id serial PRIMARY KEY, customer_id int NOT NULL REFERENCES customers(id),
                                    amount text NOT NULL, memo text);
             COMMENT ON COLUMN payments.amount IS 'Valor pago em reais, em texto (veio do sistema antigo)';
             INSERT INTO customers (name, email, city, notes) VALUES
               ('Ana {word}', '{word}.ana@exemplo.com', 'Porto Alegre', 'pediu sigilo: {word}'),
               ('Bia Souza', 'bia.{number}@exemplo.com', 'Curitiba', 'documento {number}'),
               ('Caio Lima', 'caio@exemplo.com', 'Recife', 'token {hex}');
             INSERT INTO payments (customer_id, amount, memo) VALUES
               (1, '120.50', 'pix {uuid}'), (1, '80', 'boleto {word}'), (2, '42.10', NULL),
               (3, '{word}', 'estorno');"
        ))
        .await
        .unwrap();

    let (catalogs, entry, dir) = open_catalog(&pg_server, SCRATCH).await;
    let db = AdapterDatabase {
        adapter: Arc::new(PostgresAdapter::new(&pg_server, SCRATCH).unwrap()),
        db_type: DatabaseType::Postgres,
    };

    // Erro do banco que cita um valor (`invalid input syntax … "<canário>"`) e
    // a correção na tentativa seguinte
    let first = "Quanto foi pago no total? Some a coluna amount da tabela payments convertendo direto com amount::numeric.";
    let (finish, events) = ask(&participant, &db, Structure::Catalog(&catalogs, &entry), first).await;
    assert_eq!(finish.status, AssistantTurnStatus::Answered, "{finish:?}");
    assert!(
        events.iter().any(|line| line.starts_with("StageFailed") && line.contains(word.as_str())),
        "o erro com o valor não chegou à tela: {events:?}"
    );
    assert!(corrected_attempts(&participant, 0).await >= 1);
    assert_saved(&participant, 0).await;
    let first_attempt = finish.attempt_id.clone().unwrap();

    // Pergunta de seguimento: o histórico vai junto; as linhas com os
    // canários ficam na tela
    let second = "E quanto cada cliente pagou? Mostre o nome e o e-mail de cada um.";
    let (finish, events) = ask(&participant, &db, Structure::Catalog(&catalogs, &entry), second).await;
    assert_eq!(finish.status, AssistantTurnStatus::Answered, "{finish:?}");
    assert!(events.iter().any(|line| line.starts_with("result: ") && line.contains(word.as_str())));
    assert_saved(&participant, 1).await;
    let (turn_id, second_attempt) = (finish.turn_id.clone().unwrap(), finish.attempt_id.clone().unwrap());

    // Execução manual, editada, em páginas de 2 (C-09)
    let listing = "SELECT name, email, notes FROM customers ORDER BY id";
    let page = |offset| QueryOptions {
        limit: 2,
        offset,
        ..QueryOptions::default()
    };
    let first_page = execute_manually(&participant, &db, listing, page(0), &second_attempt, true)
        .await
        .unwrap();
    assert_eq!(first_page.row_count, 2);
    assert!(first_page.has_more);
    let second_page = execute_manually(&participant, &db, listing, page(2), &second_attempt, true)
        .await
        .unwrap();
    assert_eq!(second_page.row_count, 1);
    assert!(!second_page.has_more);
    assert!(format!("{:?}", second_page.rows).contains(hex.as_str()));

    // Execução manual que falha com o valor no erro: vai sanitizada
    let failing = "SELECT sum(amount::numeric) FROM payments";
    let error = execute_manually(&participant, &db, failing, page(0), &first_attempt, true)
        .await
        .unwrap_err();
    assert!(error.to_string().contains(word.as_str()), "{error}");
    let saved = participant
        .client
        .conversation(&participant.token, &participant.conversation_id)
        .await
        .unwrap();
    let outcome = saved.turns[0].attempts.last().unwrap().outcome.clone().unwrap();
    assert_eq!(outcome.execution.ok, Some(false));
    assert!(outcome.user_edited);
    println!("erro manual na API: {:?}", outcome.execution.error_sanitized);

    // Avaliação com comentário (o texto é do usuário: vai como escrito)
    let feedback = AssistantFeedbackInput {
        rating: FeedbackRating::Down,
        reasons: vec![FeedbackReason::WrongResult],
        comment: Some("e2e B10: faltou separar por cidade".into()),
        attempt_id: Some(second_attempt.clone()),
    }
    .normalized()
    .unwrap();
    participant.sent.lock().push(format!("avaliação: {}", serde_json::to_string(&feedback).unwrap()));
    participant
        .client
        .set_feedback(&participant.token, &turn_id, &feedback)
        .await
        .unwrap();

    history_on_another_machine(&participant, &[first, second]).await;
    assert_nothing_leaked(&canaries, &participant, since).await;

    drop(entry);
    drop(db);
    drop(catalogs);
    let _ = std::fs::remove_dir_all(dir);
    admin
        .batch_execute(&format!("DROP DATABASE IF EXISTS {SCRATCH} WITH (FORCE)"))
        .await
        .unwrap();
}

/// Conexão direta ao Postgres dos e2e, para semear e apagar o banco.
async fn pg_client(database: &str) -> tokio_postgres::Client {
    let (client, connection) = tokio_postgres::connect(
        &format!("host=localhost port=5432 user=postgres password=1234 dbname={database}"),
        tokio_postgres::NoTls,
    )
    .await
    .unwrap();
    tokio::spawn(connection);
    client
}

#[tokio::test]
#[ignore = "requires the assistant API, its database and container"]
async fn e2e_privacy_sqlite_canaries() {
    capture_logs();
    let since = now_unix().saturating_sub(5);
    let Some(participant) = participant(DatabaseType::Sqlite).await else {
        return;
    };
    let canaries = Canaries::new();
    let Canaries { word, number, hex, uuid } = &canaries;

    let path = std::env::temp_dir().join(format!("{SCRATCH}.db"));
    let _ = std::fs::remove_file(&path);
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute_batch(&format!(
            "CREATE TABLE customers (id INTEGER PRIMARY KEY, name TEXT NOT NULL, email TEXT NOT NULL, notes TEXT);
             CREATE TABLE payments (id INTEGER PRIMARY KEY, customer_id INTEGER NOT NULL REFERENCES customers(id),
                                    amount REAL NOT NULL, memo TEXT);
             INSERT INTO customers (name, email, notes) VALUES
               ('Ana {word}', '{word}.ana@exemplo.com', 'pediu sigilo'),
               ('Bia Souza', 'bia.{number}@exemplo.com', 'token {hex}');
             INSERT INTO payments (customer_id, amount, memo) VALUES (1, 120.5, 'pix {uuid}'), (2, 42.1, NULL);"
        ))
        .unwrap();
    let sqlite_server = server(DatabaseType::Sqlite, 0, Some(path.to_string_lossy().into_owned()));
    let (catalogs, entry, dir) = open_catalog(&sqlite_server, "main").await;
    let db = AdapterDatabase {
        adapter: Arc::new(SqliteAdapter::new(&sqlite_server, "main").unwrap()),
        db_type: DatabaseType::Sqlite,
    };

    let (finish, events) = ask(
        &participant,
        &db,
        Structure::Catalog(&catalogs, &entry),
        "Liste os clientes com o e-mail e o total pago por cada um.",
    )
    .await;
    assert_eq!(finish.status, AssistantTurnStatus::Answered, "{finish:?}");
    assert!(events.iter().any(|line| line.starts_with("result: ") && line.contains(word.as_str())));
    assert_saved(&participant, 0).await;

    assert_nothing_leaked(&canaries, &participant, since).await;

    drop(entry);
    let _ = std::fs::remove_dir_all(dir);
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
#[ignore = "requires the assistant API, its database and container, and mongodb on localhost:27017 (no auth)"]
async fn e2e_privacy_mongo_canaries() {
    use mongodb::bson::{doc, Document};

    capture_logs();
    let since = now_unix().saturating_sub(5);
    let Some(participant) = participant(DatabaseType::Mongodb).await else {
        return;
    };
    let canaries = Canaries::new();
    let Canaries { word, number, hex, uuid } = &canaries;

    // Valores, comentário, e-mail e chaves de mapa (`tags.<uuid>`) com canário
    let mongo = mongodb::Client::with_uri_str("mongodb://localhost:27017").await.unwrap();
    let database = mongo.database(SCRATCH);
    database.drop().await.unwrap();
    database
        .collection::<Document>("payments")
        .insert_many([
            doc! { "customer": { "name": format!("Ana {word}"), "email": format!("{word}.ana@exemplo.com") },
                   "amount": "120.50", "comment": format!("pediu sigilo: {word}"), "tags": { uuid.as_str(): true } },
            doc! { "customer": { "name": "Bia Souza", "email": format!("bia.{number}@exemplo.com") },
                   "amount": "80", "comment": format!("token {hex}"), "tags": { client_turn_id(): true } },
            doc! { "customer": { "name": "Caio Lima", "email": "caio@exemplo.com" },
                   "amount": word.as_str(), "comment": "estorno", "tags": { client_turn_id(): false } },
        ])
        .await
        .unwrap();

    let mongo_server = server(DatabaseType::Mongodb, 27017, None);
    let (catalogs, entry, dir) = open_catalog(&mongo_server, SCRATCH).await;
    let db = AdapterDatabase {
        adapter: Arc::new(MongoAdapter::new(&mongo_server, SCRATCH).await.unwrap()),
        db_type: DatabaseType::Mongodb,
    };

    // `$toDouble` sem `onError` no valor que não é número: o erro cita o
    // valor, e a correção acontece na tentativa seguinte
    let (finish, events) = ask(
        &participant,
        &db,
        Structure::Catalog(&catalogs, &entry),
        "Quanto foi pago no total? Some o campo amount de payments convertendo com $toDouble, sem onError.",
    )
    .await;
    assert_eq!(finish.status, AssistantTurnStatus::Answered, "{finish:?}");
    assert!(
        events.iter().any(|line| line.starts_with("StageFailed") && line.contains(word.as_str())),
        "o erro com o valor não chegou à tela: {events:?}"
    );
    assert!(corrected_attempts(&participant, 0).await >= 1);
    assert_saved(&participant, 0).await;

    assert_nothing_leaked(&canaries, &participant, since).await;

    drop(entry);
    let _ = std::fs::remove_dir_all(dir);
    database.drop().await.unwrap();
}

#[tokio::test]
#[ignore = "requires the assistant API, its database and container, and redis on localhost:6380 (no auth)"]
async fn e2e_privacy_redis_canaries() {
    capture_logs();
    let since = now_unix().saturating_sub(5);
    let Some(participant) = participant(DatabaseType::Redis).await else {
        return;
    };
    let canaries = Canaries::new();
    let Canaries { word, number, hex, uuid } = &canaries;

    // Redis descartável (db 3; os outros são dos demais e2e). Chaves com cara
    // de registro: UUID, número, hex, e-mail e nomes demais numa posição
    let adapter = RedisAdapter::new(&server(DatabaseType::Redis, 6380, None), "3").unwrap();
    adapter.execute_statement("FLUSHDB").await.unwrap();
    let mut statements = vec![
        format!("HSET order:{uuid} customer \"Ana {word}\" total 120.5"),
        format!("HSET user:{number} name \"Bia {word}\" email \"{word}@exemplo.com\""),
        format!("SET session:{hex} \"{word}\""),
        format!("SET email:{word}@exemplo.com 1"),
    ];
    for _ in 0..4 {
        statements.push(format!("HSET order:{} customer \"Caio\" total 10", client_turn_id()));
    }
    for suffix in 'a'..='j' {
        statements.push(format!("SET customer:{word}{suffix} \"{word}\""));
    }
    for statement in &statements {
        adapter.execute_statement(statement).await.unwrap();
    }
    let db = AdapterDatabase {
        adapter: Arc::new(adapter),
        db_type: DatabaseType::Redis,
    };

    let (finish, events) = ask(&participant, &db, Structure::Keys, "Quantos pedidos existem?").await;
    assert_eq!(finish.status, AssistantTurnStatus::Answered, "{finish:?}");
    assert!(events.iter().any(|line| line.starts_with("result: ")));
    assert_saved(&participant, 0).await;

    assert_nothing_leaked(&canaries, &participant, since).await;

    db.adapter.execute_statement("FLUSHDB").await.unwrap();
}
