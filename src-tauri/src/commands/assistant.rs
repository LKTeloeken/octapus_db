use std::future::Future;
use std::sync::Arc;

use tauri::ipc::Channel;
use tauri::State;

use crate::adapters::ReadOnlyOp;
use crate::assistant::backend::ModelProfile;
use crate::assistant::intermediary::Intermediary;
use crate::assistant::journal::TurnJournal;
use crate::assistant::language;
use crate::assistant::read_only::AdapterDatabase;
use crate::assistant::requested::RequestedLookup;
use crate::assistant::schema_context::ContextRequest;
use crate::assistant::setup::{
    clean_mentions, conversation_schemas, has_catalog, needs_update, query_language, report_manual_execution,
    sample_values_option, structure_context, ManualExecution, CONVERSATIONS_PAGE, FIRST_PAGE_ROWS,
};
use crate::assistant::turn::{self, TurnPorts, TurnSetup};
use crate::assistant::{prompts, read_only, ApiClient, AssistantCode, AssistantError};
use crate::catalog::FLAT_SCHEMA;
use crate::error::{Error, Result};
use crate::models::{
    AdapterCapabilities, AssistantConsentTerm, AssistantConversation, AssistantConversationPage,
    AssistantConversationSummary, AssistantFeedbackInput, AssistantMe, AssistantModel,
    AssistantParticipant, AssistantStatus, AssistantTurnAskInput, AssistantTurnEvent, AssistantTurnFinish,
    DatabaseType, QueryOptions, QueryResult,
};
use crate::state::AppState;
use crate::storage::repositories::assistant as assistant_store;
use crate::storage::repositories::{servers, settings as settings_store};

use super::catalog::entry_for;
use super::connect_adapter;

/// Tamanho máximo da resposta a uma pergunta do modelo (o mesmo teto da API).
const USER_REPLY_MAX: usize = 2000;

/// A URL e o cliente da API — só para quem está no canal beta
/// (`AppSettings::beta_updates`). Enquanto o assistente depende de convite,
/// a versão estável não o mostra nem fala com a API: fora do beta tudo
/// responde como um build sem a URL (`ASSISTANT_NOT_CONFIGURED`).
fn endpoint(state: &AppState) -> Result<(&str, &ApiClient)> {
    if !settings_store::load(&state.storage)?.beta_updates {
        return Err(Error::AssistantNotConfigured);
    }
    state.assistant.endpoint()
}

/// Configurado? A API está no ar? Há login? Nunca rejeita por causa da API:
/// sem URL ou com a API fora, o status diz isso e o resto do app segue (C-19).
#[tauri::command]
pub async fn assistant_status(state: State<'_, AppState>) -> Result<AssistantStatus> {
    let Ok((api_url, client)) = endpoint(&state) else {
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
    let (api_url, client) = endpoint(&state)?;
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
/// O `clientTurnId` do [`AssistantTurnAskInput`] (UUID do front) torna a
/// abertura idempotente e identifica o turno para o `assistant_turn_cancel`.
/// A API intermediária é quem gera e quem guarda o turno ([`Intermediary`]).
#[tauri::command]
pub async fn assistant_turn_ask(
    state: State<'_, AppState>,
    input: AssistantTurnAskInput,
    on_event: Channel<AssistantTurnEvent>,
) -> Result<AssistantTurnFinish> {
    let AssistantTurnAskInput {
        server_id,
        database,
        conversation_id,
        question,
        client_turn_id,
        schemas,
        mentions,
        past_mentions,
    } = input;
    let question = question.trim().to_string();
    if question.is_empty() {
        return Err(Error::InvalidQuery("Empty question".into()));
    }
    let db_type = assistant_db_type(&state, server_id)?;
    let sample_values = servers::get_by_id_meta(&state.storage, server_id)?.assistant_sample_values;
    let prompt = prompts::for_db(db_type).expect("banco com assistente tem prompt");
    let auto_execute = settings_store::load(&state.storage)?.assistant_auto_execute;
    let (api_url, client) = endpoint(&state)?;
    let token = assistant_store::load_token(&state.storage, api_url)?.ok_or_else(not_signed_in)?;

    let api = Intermediary {
        client,
        token: &token,
        assistant: &state.assistant,
    };

    let handle = state.assistant.start_turn(&client_turn_id);
    let result = async {
        let conversation = api
            .conversation(&conversation_id)
            .await
            .inspect_err(|error| unauthorized_clears(&state, error))?;
        if Some(conversation.summary.paradigm.as_str()) != prompts::paradigm(db_type) {
            return Err(Error::InvalidState(
                "the conversation belongs to another database type".into(),
            ));
        }

        let schemas = conversation_schemas(db_type, schemas);
        let mentions = clean_mentions(mentions);
        let past_mentions = clean_mentions(past_mentions);
        let history = turn::history_from(&conversation);
        let past_questions: Vec<String> = history.iter().map(|turn| turn.question.clone()).collect();
        let model = ModelProfile::from(&conversation.summary.model);
        let adapter = connect_adapter(&state, server_id, &database).await?;
        let entry = match has_catalog(db_type) {
            true => Some(entry_for(&state, server_id, &database).await?),
            false => None,
        };
        let schema = structure_context(
            &state.catalog,
            entry.as_ref(),
            db_type,
            adapter.as_ref(),
            ContextRequest {
                schemas: &schemas,
                question: &question,
                history: &past_questions,
                context_window: model.context_window,
                mentions: &mentions,
                past_mentions: &past_mentions,
                sample_values,
            },
        )
        .await?;

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
            model,
            auto_execute,
            sample_values: sample_values_option(db_type, sample_values),
            reply_timeout: turn::REPLY_TIMEOUT,
            page: QueryOptions {
                limit: FIRST_PAGE_ROWS,
                schema: schemas.first().filter(|schema| schema.as_str() != FLAT_SCHEMA).cloned(),
                query_id: Some(client_turn_id.clone()),
                ..QueryOptions::default()
            },
        };
        let lookup = RequestedLookup {
            catalogs: &state.catalog,
            entry,
            adapter: Arc::clone(&adapter),
            db_type,
            scope: &schemas,
            sample_values,
        };
        let db = AdapterDatabase { adapter, db_type };
        let ports = TurnPorts {
            backend: &api,
            journal: &api,
            db: &db,
            structure: &lookup,
        };
        let emit = |event: AssistantTurnEvent| {
            // Aba fechada: ninguém ouvindo, o turno segue até o fim
            let _ = on_event.send(event);
        };
        turn::run_turn(&ports, &setup, &emit, &handle.cancel, &handle.reply).await
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

/// Responde à pergunta que o modelo fez no turno em andamento (evento
/// `question`, `kind: ask_user`); o turno segue com a resposta. `false` se o
/// turno já terminou ou não está esperando resposta. A resposta vai ao modelo
/// e à API como o usuário escreveu, como a própria pergunta do turno.
#[tauri::command]
pub fn assistant_turn_reply(state: State<'_, AppState>, client_turn_id: String, reply: String) -> Result<bool> {
    let reply = reply.trim();
    if reply.is_empty() {
        return Err(Error::InvalidQuery("Empty reply".into()));
    }
    if reply.chars().count() > USER_REPLY_MAX {
        return Err(Error::InvalidQuery(format!("The reply is longer than {USER_REPLY_MAX} characters")));
    }
    Ok(state.assistant.reply_turn(&client_turn_id, reply.to_string()))
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
        if let Ok((api_url, client)) = endpoint(&state) {
            if let Ok(Some(token)) = assistant_store::load_token(&state.storage, api_url) {
                let execution = ManualExecution {
                    result: &result,
                    query: &query,
                    duration,
                    user_edited,
                };
                report_manual_execution(&state.assistant, client, &token, attempt_id, execution).await;
            }
        }
    }
    result
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

/// Chamada autenticada com o token salvo para a API atual. Sem token →
/// `ASSISTANT_UNAUTHORIZED`, o mesmo de um token recusado: para a tela, os dois
/// pedem o convite. Um 401 da API (token inválido, convite revogado) apaga o
/// token local, e o `assistant_status` volta a dizer "sem login".
async fn authorized<'a, T, F, Fut>(state: &'a AppState, operation: &str, call: F) -> Result<T>
where
    F: FnOnce(&'a ApiClient, String) -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let (api_url, client) = endpoint(state)?;
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
