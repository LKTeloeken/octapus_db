use std::future::Future;
use std::sync::Arc;

use async_trait::async_trait;
use tauri::ipc::Channel;
use tauri::State;

use crate::adapters::{DatabaseAdapter, ReadOnlyOp};
use crate::assistant::client::{AttemptProgress, AttemptReply};
use crate::assistant::failure::{classify, sanitize};
use crate::assistant::language::{self, Language};
use crate::assistant::messages::AttemptRequest;
use crate::assistant::schema_context::{self, ContextRequest};
use crate::assistant::turn::{self, TurnApi, TurnDatabase, TurnSetup};
use crate::assistant::{prompts, read_only, ApiClient, AssistantCode, AssistantError, AssistantService, CancelFlag};
use crate::catalog::FLAT_SCHEMA;
use crate::error::{Error, Result};
use crate::models::{
    AdapterCapabilities, AssistantConsentTerm, AssistantConversation, AssistantConversationPage,
    AssistantConversationSummary, AssistantFeedbackInput, AssistantMe, AssistantModel,
    AssistantParticipant, AssistantStatus, AssistantTurnEvent, AssistantTurnFinish, AssistantTurnStatus,
    AssistantTurnSummary, AttemptOutcome, DatabaseType, QueryLanguage, QueryOptions, QueryResult,
    ReadOnlyPlan, StageResult,
};
use crate::state::AppState;
use crate::storage::repositories::assistant as assistant_store;
use crate::storage::repositories::{servers, settings as settings_store};

use super::catalog::entry_for;
use super::connect_adapter;

/// Primeira página do resultado do assistente.
const FIRST_PAGE_ROWS: i64 = 200;
/// Conversas por página na lista.
const CONVERSATIONS_PAGE: u32 = 30;

/// Configurado? A API está no ar? Há login? Nunca rejeita por causa da API:
/// sem URL ou com a API fora, o status diz isso e o resto do app segue (C-19).
#[tauri::command]
pub async fn assistant_status(state: State<'_, AppState>) -> Result<AssistantStatus> {
    let Ok((api_url, client)) = state.assistant.endpoint() else {
        return Ok(AssistantStatus::not_configured());
    };
    let logged_in = assistant_store::load_token(&state.storage, api_url)?.is_some();

    let health = match client.health().await {
        Ok(health) => Some(health),
        Err(error) => {
            log::info!("assistente: API fora do ar ({})", error.code());
            None
        }
    };
    let online = health.as_ref().is_some_and(|health| health.status == "ok");
    let min_app_version = health.and_then(|health| health.min_app_version);

    Ok(AssistantStatus {
        configured: true,
        online,
        logged_in,
        update_required: needs_update(min_app_version.as_deref(), env!("CARGO_PKG_VERSION")),
        min_app_version,
    })
}

/// Troca o código de convite por um token de participante e o guarda cifrado.
#[tauri::command]
pub async fn assistant_redeem(
    state: State<'_, AppState>,
    invite_code: String,
    device_label: Option<String>,
) -> Result<AssistantParticipant> {
    let (api_url, client) = state.assistant.endpoint()?;
    let redeemed = client
        .redeem(invite_code.trim(), device_label.as_deref())
        .await
        .inspect_err(|error| log::warn!("assistente: convite recusado ({})", error.code()))?;

    assistant_store::save_token(&state.storage, api_url, &redeemed.token)?;
    log::info!("assistente: login do participante {}", redeemed.participant.id);
    Ok(redeemed.participant)
}

/// Esquece o token nesta máquina (a API não tem logout).
#[tauri::command]
pub fn assistant_logout(state: State<'_, AppState>) -> Result<()> {
    assistant_store::clear(&state.storage)?;
    log::info!("assistente: logout");
    Ok(())
}

/// Participante, consentimento, cota e modo de seleção de modelo.
#[tauri::command]
pub async fn assistant_me(state: State<'_, AppState>) -> Result<AssistantMe> {
    authorized(&state, "me", |client, token| async move { client.me(&token).await }).await
}

/// Texto do termo vigente, para exibir antes do aceite (RN-11).
#[tauri::command]
pub async fn assistant_consent_current(state: State<'_, AppState>) -> Result<AssistantConsentTerm> {
    authorized(&state, "termo", |client, token| async move {
        client.consent_current(&token).await
    })
    .await
}

/// Aceita o termo `version` (precisa ser o vigente: outro → `ASSISTANT_CONFLICT`).
#[tauri::command]
pub async fn assistant_consent_accept(state: State<'_, AppState>, version: String) -> Result<()> {
    authorized(&state, "aceite do termo", |client, token| async move {
        client.consent_accept(&token, &version).await
    })
    .await?;
    log::info!("assistente: termo aceito");
    Ok(())
}

/// Modelos que o participante pode escolher — só no modo `free`; nos outros a
/// API responde `ASSISTANT_MODEL_SELECTION_FORBIDDEN`.
#[tauri::command]
pub async fn assistant_models(state: State<'_, AppState>) -> Result<Vec<AssistantModel>> {
    authorized(&state, "modelos", |client, token| async move {
        client.models(&token).await
    })
    .await
}

/// Conversas do participante, mais recentes primeiro.
#[tauri::command]
pub async fn assistant_conversations(
    state: State<'_, AppState>,
    cursor: Option<String>,
) -> Result<AssistantConversationPage> {
    authorized(&state, "conversas", |client, token| async move {
        client.conversations(&token, cursor.as_deref(), CONVERSATIONS_PAGE).await
    })
    .await
}

/// A conversa inteira, com turnos, tentativas e avaliações (C-15).
#[tauri::command]
pub async fn assistant_conversation(
    state: State<'_, AppState>,
    conversation_id: String,
) -> Result<AssistantConversation> {
    authorized(&state, "conversa", |client, token| async move {
        client.conversation(&token, &conversation_id).await
    })
    .await
}

/// Abre uma conversa para o tipo de banco do servidor. O modelo é decidido
/// aqui (sorteado, fixo ou o escolhido no modo `free`) e não muda mais.
#[tauri::command]
pub async fn assistant_conversation_create(
    state: State<'_, AppState>,
    server_id: i64,
    model_id: Option<String>,
    title: Option<String>,
) -> Result<AssistantConversationSummary> {
    let db_type = assistant_db_type(&state, server_id)?;
    let paradigm = prompts::paradigm(db_type).expect("banco com assistente tem paradigma");
    let conversation = authorized(&state, "nova conversa", |client, token| async move {
        client
            .create_conversation(&token, paradigm, model_id.as_deref(), title.as_deref())
            .await
    })
    .await?;
    log::info!("assistente: conversa {} aberta ({paradigm})", conversation.id);
    Ok(conversation)
}

/// Pergunta: roda o turno inteiro (B6) e manda o andamento pelo `onEvent`.
/// `clientTurnId` (UUID do front) torna a abertura idempotente e identifica o
/// turno para o `assistant_turn_cancel`. `schemas`: os da conversa no
/// Postgres (o primeiro vai no `search_path`); vazio = `public`.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn assistant_turn_ask(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
    conversation_id: String,
    question: String,
    client_turn_id: String,
    schemas: Option<Vec<String>>,
    on_event: Channel<AssistantTurnEvent>,
) -> Result<AssistantTurnFinish> {
    let question = question.trim().to_string();
    if question.is_empty() {
        return Err(Error::InvalidQuery("Empty question".into()));
    }
    let db_type = assistant_db_type(&state, server_id)?;
    let prompt = prompts::for_db(db_type).expect("banco com assistente tem prompt");
    let auto_execute = settings_store::load(&state.storage)?.assistant_auto_execute;
    let (api_url, client) = state.assistant.endpoint()?;
    let token = assistant_store::load_token(&state.storage, api_url)?.ok_or_else(not_signed_in)?;

    let cancel = state.assistant.start_turn(&client_turn_id);
    let result = async {
        let conversation = client
            .conversation(&token, &conversation_id)
            .await
            .inspect_err(|error| unauthorized_clears(&state, error))?;
        if Some(conversation.summary.paradigm.as_str()) != prompts::paradigm(db_type) {
            return Err(Error::InvalidState(
                "the conversation belongs to another database type".into(),
            ));
        }

        let schemas = conversation_schemas(db_type, schemas);
        let entry = entry_for(&state, server_id, &database).await?;
        let history = turn::history_from(&conversation);
        let past_questions: Vec<String> = history.iter().map(|turn| turn.question.clone()).collect();
        let context_window = conversation.summary.model.context_window;
        let schema = schema_context::build(
            &state.catalog,
            &entry,
            db_type,
            ContextRequest {
                schemas: &schemas,
                question: &question,
                history: &past_questions,
                context_window,
            },
        )
        .await?;
        let adapter = connect_adapter(&state, server_id, &database).await?;

        let setup = TurnSetup {
            conversation_id: conversation_id.clone(),
            client_turn_id: client_turn_id.clone(),
            language: language::detect(&question),
            question: question.clone(),
            prompt,
            db_type,
            expected_language: query_language(db_type),
            schema,
            history,
            context_window,
            auto_execute,
            page: QueryOptions {
                limit: FIRST_PAGE_ROWS,
                schema: schemas.first().filter(|schema| schema.as_str() != FLAT_SCHEMA).cloned(),
                query_id: Some(client_turn_id.clone()),
                ..QueryOptions::default()
            },
        };
        let api = ApiTurns {
            client,
            token: &token,
            assistant: &state.assistant,
        };
        let db = AdapterDatabase { adapter, db_type };
        let emit = |event: AssistantTurnEvent| {
            // Aba fechada: ninguém ouvindo, o turno segue até o fim
            let _ = on_event.send(event);
        };
        turn::run_turn(&api, &db, &setup, &emit, &cancel).await
    }
    .await;
    state.assistant.end_turn(&client_turn_id);

    match &result {
        Ok(finish) if finish.error_code.as_deref() == Some("ASSISTANT_UNAUTHORIZED") => {
            assistant_store::clear(&state.storage)?;
        }
        Err(error) => {
            unauthorized_clears(&state, error);
            log::warn!("assistente: turno não abriu ({})", error.code());
        }
        Ok(_) => {}
    }
    result
}

/// Cancela o turno em andamento (C-18): fecha o stream do modelo (a API
/// devolve a cota) e interrompe a consulta no banco. `false` se já terminou.
#[tauri::command]
pub fn assistant_turn_cancel(state: State<'_, AppState>, client_turn_id: String) -> Result<bool> {
    Ok(state.assistant.cancel_turn(&client_turn_id))
}

/// Execução manual de uma consulta do assistente: na revisão (execução
/// automática desligada), depois de editar, e as páginas seguintes (C-09).
/// Sempre pelo caminho só-leitura (guard + banco). Com `attemptId`, a
/// primeira página completa o outcome da tentativa na API.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn assistant_execute(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
    query: String,
    page: QueryOptions,
    attempt_id: Option<String>,
    user_edited: bool,
) -> Result<QueryResult> {
    let db_type = assistant_db_type(&state, server_id)?;
    let adapter = connect_adapter(&state, server_id, &database).await?;
    let op = ReadOnlyOp::new(query_language(db_type), query.clone());
    let offset = page.offset;

    let started = std::time::Instant::now();
    let result = read_only::execute(adapter.as_ref(), db_type, &op, page).await;
    let duration = Some(started.elapsed().as_millis() as i64);

    if let (Some(attempt_id), 0) = (attempt_id.as_deref(), offset) {
        report_manual_execution(&state, attempt_id, &result, &query, duration, user_edited).await;
    }
    result
}

/// Completa o outcome guardado da tentativa com a execução manual e manda de
/// novo (repetir sobrescreve). Sem o outcome em memória ou sem login, não faz
/// nada: é registro do experimento, não pode falhar a execução.
async fn report_manual_execution(
    state: &State<'_, AppState>,
    attempt_id: &str,
    result: &Result<QueryResult>,
    query: &str,
    duration: Option<i64>,
    user_edited: bool,
) {
    let Some(mut outcome) = state.assistant.outcome(attempt_id) else {
        return;
    };
    match result {
        Ok(result) => {
            outcome.execution = StageResult {
                ran: true,
                ok: Some(true),
                failure_class: None,
                error_sanitized: None,
                duration_ms: duration,
            };
            outcome.row_count = Some(result.row_count as i64);
        }
        // Recusa do guard na revisão: a consulta editada não passou
        Err(Error::GuardRejected(_)) => return,
        Err(error) => {
            let class = classify(error);
            outcome.execution = StageResult {
                ran: true,
                ok: Some(false),
                failure_class: class,
                error_sanitized: class.and(sanitize(error, query)),
                duration_ms: duration,
            };
        }
    }
    outcome.user_edited = user_edited;

    let Ok((api_url, client)) = state.assistant.endpoint() else {
        return;
    };
    let Ok(Some(token)) = assistant_store::load_token(&state.storage, api_url) else {
        return;
    };
    match client.report_outcome(&token, attempt_id, &outcome).await {
        Ok(()) => state.assistant.remember_outcome(attempt_id, &outcome),
        Err(error) => log::warn!(
            "assistente: outcome da execução manual de {attempt_id} não foi registrado ({})",
            error.code()
        ),
    }
}

/// Avalia a resposta do turno (👍/👎, motivos e comentário); repetir
/// sobrescreve. Comentário acima do teto → `ASSISTANT_INVALID_REQUEST` sem ir
/// à API. O comentário nunca vai para o log.
#[tauri::command]
pub async fn assistant_feedback_set(
    state: State<'_, AppState>,
    turn_id: String,
    feedback: AssistantFeedbackInput,
) -> Result<()> {
    let feedback = feedback
        .normalized()
        .map_err(|message| Error::from(AssistantError::new(AssistantCode::InvalidRequest, message)))?;
    let rating = feedback.rating;
    let turn = turn_id.as_str();
    authorized(&state, "avaliação", |client, token| async move {
        client.set_feedback(&token, turn, &feedback).await
    })
    .await?;
    log::info!("assistente: avaliação do turno {turn_id} gravada ({rating:?})");
    Ok(())
}

/// Remove a avaliação do turno (sem avaliação não é erro).
#[tauri::command]
pub async fn assistant_feedback_clear(state: State<'_, AppState>, turn_id: String) -> Result<()> {
    let turn = turn_id.as_str();
    authorized(&state, "remover avaliação", |client, token| async move {
        client.clear_feedback(&token, turn).await
    })
    .await?;
    log::info!("assistente: avaliação do turno {turn_id} removida");
    Ok(())
}

/// O tipo do banco do servidor, se o assistente atende (capabilities).
fn assistant_db_type(state: &State<'_, AppState>, server_id: i64) -> Result<DatabaseType> {
    let db_type = servers::get_by_id_meta(&state.storage, server_id)?.db_type;
    let supported = AdapterCapabilities::for_db_type(db_type).is_some_and(|caps| caps.supports_assistant);
    if !supported {
        return Err(Error::UnsupportedType(format!(
            "the assistant does not support {db_type:?} yet"
        )));
    }
    Ok(db_type)
}

/// Em que linguagem o assistente escreve para o banco (capabilities).
fn query_language(db_type: DatabaseType) -> QueryLanguage {
    AdapterCapabilities::for_db_type(db_type).map_or(QueryLanguage::Sql, |caps| caps.query_language)
}

/// Os schemas da conversa: no SQLite e no Mongo, o schema sem nome; no
/// Postgres, os pedidos (sem repetir) ou o `public`.
fn conversation_schemas(db_type: DatabaseType, requested: Option<Vec<String>>) -> Vec<String> {
    if db_type != DatabaseType::Postgres {
        return vec![FLAT_SCHEMA.to_string()];
    }
    let mut schemas: Vec<String> = Vec::new();
    for schema in requested.unwrap_or_default() {
        let schema = schema.trim().to_string();
        if !schema.is_empty() && !schemas.contains(&schema) {
            schemas.push(schema);
        }
    }
    if schemas.is_empty() {
        schemas.push("public".to_string());
    }
    schemas
}

fn not_signed_in() -> Error {
    Error::from(AssistantError::new(AssistantCode::Unauthorized, "not signed in"))
}

/// Um 401 da API (convite revogado) apaga o token local.
fn unauthorized_clears(state: &State<'_, AppState>, error: &Error) {
    if matches!(error, Error::Assistant(e) if e.code == AssistantCode::Unauthorized) {
        let _ = assistant_store::clear(&state.storage);
        log::info!("assistente: token recusado pela API, sessão local apagada");
    }
}

/// A API do turno, com o token do participante. O outcome enviado fica em
/// memória para a execução manual completar depois.
struct ApiTurns<'a> {
    client: &'a ApiClient,
    token: &'a str,
    assistant: &'a AssistantService,
}

#[async_trait]
impl TurnApi for ApiTurns<'_> {
    async fn create_turn(
        &self,
        conversation_id: &str,
        question: &str,
        language: Language,
        client_turn_id: &str,
    ) -> Result<AssistantTurnSummary> {
        self.client
            .create_turn(self.token, conversation_id, question, language, client_turn_id)
            .await
    }

    async fn create_attempt(
        &self,
        turn_id: &str,
        request: &AttemptRequest,
        cancel: &CancelFlag,
        progress: &(dyn Fn(AttemptProgress) + Send + Sync),
    ) -> Result<AttemptReply> {
        self.client
            .create_attempt(self.token, turn_id, request, cancel, progress)
            .await
    }

    async fn report_outcome(&self, attempt_id: &str, outcome: &AttemptOutcome) -> Result<()> {
        self.client.report_outcome(self.token, attempt_id, outcome).await?;
        self.assistant.remember_outcome(attempt_id, outcome);
        Ok(())
    }

    async fn finish_turn(&self, turn_id: &str, status: AssistantTurnStatus) -> Result<()> {
        self.client.finish_turn(self.token, turn_id, status).await
    }
}

/// O banco do usuário pelo caminho só-leitura do assistente (B2/B3): guard
/// de novo (o turno já conferiu; aqui é a mesma porta da execução manual) e
/// as barreiras do adapter.
struct AdapterDatabase {
    adapter: Arc<dyn DatabaseAdapter>,
    db_type: DatabaseType,
}

#[async_trait]
impl TurnDatabase for AdapterDatabase {
    async fn preflight(&self, op: &ReadOnlyOp, page: QueryOptions) -> Result<ReadOnlyPlan> {
        read_only::preflight(self.adapter.as_ref(), self.db_type, op, page).await
    }

    async fn execute(&self, op: &ReadOnlyOp, page: QueryOptions) -> Result<QueryResult> {
        read_only::execute(self.adapter.as_ref(), self.db_type, op, page).await
    }

    async fn cancel(&self, query_id: &str) {
        if let Err(error) = self.adapter.cancel_query(query_id).await {
            log::info!("assistente: cancelamento no banco não aplicado ({})", error.code());
        }
    }
}

/// Chamada autenticada com o token salvo para a API atual. Sem token →
/// `ASSISTANT_UNAUTHORIZED`, o mesmo de um token recusado: para a tela, os dois
/// pedem o convite. Um 401 da API (token inválido, convite revogado) apaga o
/// token local, e o `assistant_status` volta a dizer "sem login".
async fn authorized<'a, T, F, Fut>(state: &'a AppState, operation: &str, call: F) -> Result<T>
where
    F: FnOnce(&'a ApiClient, String) -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let (api_url, client) = state.assistant.endpoint()?;
    let token = assistant_store::load_token(&state.storage, api_url)?.ok_or_else(not_signed_in)?;

    let result = call(client, token).await;
    if let Err(error) = &result {
        log::warn!("assistente: {operation} falhou ({})", error.code());
        if matches!(error, Error::Assistant(e) if e.code == AssistantCode::Unauthorized) {
            assistant_store::clear(&state.storage)?;
            log::info!("assistente: token recusado pela API, sessão local apagada");
        }
    }
    result
}

/// Este app está abaixo da versão mínima que a API pede. Versão ilegível de
/// qualquer lado não bloqueia.
fn needs_update(min_app_version: Option<&str>, current: &str) -> bool {
    let (Some(min), Ok(current)) = (min_app_version, semver::Version::parse(current)) else {
        return false;
    };
    semver::Version::parse(min.trim_start_matches('v')).is_ok_and(|min| current < min)
}

#[cfg(test)]
mod tests {
    use super::needs_update;

    #[test]
    fn update_required_only_below_the_minimum() {
        assert!(needs_update(Some("1.2.0"), "1.1.0-beta.10"));
        assert!(needs_update(Some("1.1.0"), "1.1.0-beta.10"));
        assert!(!needs_update(Some("1.1.0-beta.10"), "1.1.0-beta.10"));
        assert!(!needs_update(Some("1.0.0"), "1.1.0-beta.10"));
        assert!(!needs_update(None, "1.1.0"));
        assert!(!needs_update(Some("garbage"), "1.1.0"));
    }
}

/// Um turno de verdade contra uma API do assistente (a local, com a chave de
/// teste): convite → termo → conversa → contexto de um SQLite temporário →
/// turno com o modelo real. Gasta tokens; só roda com as variáveis:
///
/// ```text
/// OCTAPUS_E2E_ASSISTANT_URL=http://localhost:8787 OCTAPUS_E2E_INVITE=<convite> \
///   cargo test --lib commands::assistant::e2e -- --ignored --nocapture
/// ```
#[cfg(test)]
mod e2e {
    use std::sync::Arc;

    use parking_lot::Mutex;

    use super::*;
    use crate::adapters::sqlite::SqliteAdapter;
    use crate::models::{Server, SslMode};
    use crate::services::{CatalogService, SourceOpener};
    use crate::storage::vault;

    #[tokio::test]
    #[ignore]
    async fn e2e_real_turn_against_the_assistant_api() {
        let (Ok(url), Ok(invite)) = (
            std::env::var("OCTAPUS_E2E_ASSISTANT_URL"),
            std::env::var("OCTAPUS_E2E_INVITE"),
        ) else {
            eprintln!("sem OCTAPUS_E2E_ASSISTANT_URL/OCTAPUS_E2E_INVITE: nada a fazer");
            return;
        };
        vault::init_for_tests();

        // Participante com o termo aceito
        let client = ApiClient::new(&url).unwrap();
        let token = client.redeem(&invite, Some("octapus e2e")).await.unwrap().token;
        let me = client.me(&token).await.unwrap();
        if let Some(current) = me.consent.current_version.as_deref() {
            if me.consent.accepted_version.as_deref() != Some(current) {
                client.consent_accept(&token, current).await.unwrap();
            }
        }
        let summary = client.create_conversation(&token, "sqlite", None, Some("e2e")).await.unwrap();

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
        let server = Server {
            id: Some(1),
            name: "e2e".into(),
            db_type: DatabaseType::Sqlite,
            host: String::new(),
            port: 0,
            username: String::new(),
            password: String::new(),
            default_database: None,
            ssl_mode: SslMode::Disable,
            ssl_root_cert: None,
            connection_uri: Some(path.to_string_lossy().into_owned()),
            scope_databases: None,
            scope_schemas: None,
            group_id: None,
            created_at: 0,
        };

        let catalogs = CatalogService::default();
        let dir = std::env::temp_dir().join(format!("octapus-assistant-e2e-{}", std::process::id()));
        catalogs.init(dir.clone(), Arc::new(|_| {}));
        let opener_server = server.clone();
        let opener: SourceOpener = Arc::new(move || {
            let server = opener_server.clone();
            Box::pin(async move { crate::adapters::create_catalog_source(&server, "main").await })
        });
        let entry = catalogs.open(1, "main", "e2e".into(), opener).await;

        let question = "Qual o total gasto por cada cliente?";
        let conversation = client.conversation(&token, &summary.id).await.unwrap();
        let schemas = [FLAT_SCHEMA.to_string()];
        let schema = schema_context::build(
            &catalogs,
            &entry,
            DatabaseType::Sqlite,
            ContextRequest {
                schemas: &schemas,
                question,
                history: &[],
                context_window: conversation.summary.model.context_window,
            },
        )
        .await
        .unwrap();

        let assistant = AssistantService::from_env();
        let setup = TurnSetup {
            conversation_id: summary.id.clone(),
            client_turn_id: "6f1c2b8e-4d3a-4e5f-9a7b-1c2d3e4f5a6b".into(),
            question: question.into(),
            language: language::detect(question),
            prompt: prompts::for_db(DatabaseType::Sqlite).unwrap(),
            db_type: DatabaseType::Sqlite,
            expected_language: QueryLanguage::Sql,
            schema,
            history: vec![],
            context_window: conversation.summary.model.context_window,
            auto_execute: true,
            page: QueryOptions {
                limit: FIRST_PAGE_ROWS,
                ..QueryOptions::default()
            },
        };
        let api = ApiTurns {
            client: &client,
            token: &token,
            assistant: &assistant,
        };
        let db = AdapterDatabase {
            adapter: Arc::new(SqliteAdapter::new(&server, "main").unwrap()),
            db_type: DatabaseType::Sqlite,
        };
        let events: Mutex<Vec<String>> = Mutex::default();
        let finish = turn::run_turn(
            &api,
            &db,
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
        )
        .await
        .unwrap();

        for line in events.lock().iter() {
            println!("{line}");
        }
        println!("fim: {:?} {:?}", finish.status, finish.error_code);
        assert_eq!(finish.status, AssistantTurnStatus::Answered, "{finish:?}");
        assert!(events.lock().iter().any(|line| line.starts_with("result: 2 linha")));

        // A API guardou o turno com a tentativa e o outcome
        let saved = client.conversation(&token, &summary.id).await.unwrap();
        let attempt = saved.turns[0].attempts.last().unwrap();
        assert!(attempt.outcome.as_ref().is_some_and(|outcome| outcome.execution.ok == Some(true)));

        drop(entry);
        let _ = std::fs::remove_dir_all(dir);
        let _ = std::fs::remove_file(path);
    }
}
