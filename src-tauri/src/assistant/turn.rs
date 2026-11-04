//! O orquestrador de um turno (B6): a pergunta vira tentativas até uma
//! resposta, um esclarecimento, código para o usuário ou o limite.
//!
//! ```text
//! abrir o turno ─▶ tentativa (SSE) ─▶ resposta conferida
//!   clarification ─▶ encerra          code ─▶ entrega, nunca executa (RN-14)
//!   answer ─▶ encerra (texto, nada executa)
//!   structure_request ─▶ o app busca os nomes no catálogo ─▶ nova tentativa
//!                        (não conta no limite, até MAX_STRUCTURE_REQUESTS)
//!   ask_user ─▶ a tela mostra a pergunta e espera a resposta ─▶ nova tentativa
//!               (não conta no limite, até MAX_USER_QUESTIONS; sem resposta,
//!               encerra como clarification)
//!   query ─▶ guard ─▶ preflight ─▶ execução (só com a automática ligada)
//!            └── falhou: outcome + erro sanitizado ─▶ nova tentativa (C-11)
//!   saída fora do formato ─▶ o texto do modelo e o motivo ─▶ nova tentativa
//! limite da API (turn_limit_exceeded) ─▶ entrega a última consulta (RN-24)
//! ```
//!
//! Cada tentativa com resposta ganha um outcome no journal. O loop depende de
//! quatro portas ([`TurnPorts`]) — quem gera ([`ModelBackend`]), onde o turno
//! fica registrado ([`TurnJournal`]), o banco ([`TurnDatabase`]) e a estrutura
//! pedida pelo modelo ([`TurnStructure`]) —, o que deixa os testes rodarem com
//! as quatro simuladas; a resposta do usuário chega pelo [`ReplySlot`]. Log só
//! com ids, contagens e códigos (nunca a pergunta ou a resposta).

use std::time::{Duration, Instant};

use async_trait::async_trait;

use super::backend::{AttemptProgress, Generation, ModelBackend, ModelProfile, StopReason};
use super::cancel::CancelFlag;
use super::journal::TurnJournal;
use super::reply::ReplySlot;
use super::error::AssistantCode;
use super::failure::{classify, sanitize, FailureClass};
use super::guard;
use super::language::Detected;
use super::messages::{
    attempt_request, AttemptRequest, Correction, HistoryTurn, InvalidRound, QuestionReply, QuestionRound, Round,
    StructureReply, StructureRound,
};
use super::output::{self, ModelOutput, OutputKind};
use super::prompts::Prompt;
use super::requested::RequestedStructure;
use super::schema_context::{budget_tokens, SchemaContext};
use crate::adapters::ReadOnlyOp;
use crate::error::{Error, Result};
use crate::models::{
    AssistantAttemptStatus, AssistantConversation, AssistantStage, AssistantTurnEvent,
    AssistantTurnFinish, AssistantTurnStatus, AttemptOutcome, DatabaseType,
    GuardResult, QueryLanguage, QueryOptions, QueryResult, ReadOnlyPlan, StageResult,
};

/// Teto de tentativas do lado do app: a API corta antes (4 por turno, com as
/// correções), mas o loop não pode depender só dela.
const MAX_ATTEMPTS: usize = 6;
/// Pedidos de estrutura por turno que não contam como tentativa (o mesmo
/// `TURN_MAX_STRUCTURE_REQUESTS` padrão da API). O seguinte não busca nada,
/// conta como tentativa e o modelo é avisado de que acabou: sem laço.
pub const MAX_STRUCTURE_REQUESTS: usize = 3;
/// Perguntas ao usuário por turno que não contam como tentativa (o
/// `TURN_MAX_USER_QUESTIONS` padrão da API). A seguinte não é mostrada: conta
/// como tentativa e o modelo é avisado de que não dá para perguntar mais.
pub const MAX_USER_QUESTIONS: usize = 2;
/// Quanto esperar a resposta do usuário antes de encerrar o turno como
/// `clarification` (a API não conta essa espera no tempo do turno).
pub const REPLY_TIMEOUT: Duration = Duration::from_secs(30 * 60);
/// Rate limit antes do stream: quantas vezes esperar e tentar de novo.
const RATE_LIMIT_RETRIES: usize = 3;
/// Espera sem `Retry-After`.
const RATE_LIMIT_DEFAULT_WAIT: Duration = Duration::from_secs(5);
/// Depois de pedir o cancelamento ao banco, quanto esperar a consulta parar
/// antes de largá-la (o SQLite só para no próprio teto de tempo).
const CANCEL_GRACE: Duration = Duration::from_secs(2);

/// O banco do usuário, só leitura (as barreiras da B2 ficam no adapter).
#[async_trait]
pub trait TurnDatabase: Send + Sync {
    async fn preflight(&self, op: &ReadOnlyOp, page: QueryOptions) -> Result<ReadOnlyPlan>;
    async fn execute(&self, op: &ReadOnlyOp, page: QueryOptions) -> Result<QueryResult>;
    /// Interrompe a execução marcada com `query_id` (o `cancel_query` do app).
    async fn cancel(&self, query_id: &str);
}

/// A estrutura que o modelo pede (`kind: structure_request`), buscada pelo app
/// só no catálogo ou nos padrões de chave (`requested.rs`): o modelo escolhe
/// os nomes, nunca a consulta de introspecção.
#[async_trait]
pub trait TurnStructure: Send + Sync {
    /// Os nomes pedidos, dentro de `budget_tokens`.
    async fn lookup(&self, names: &[String], budget_tokens: usize) -> Result<RequestedStructure>;
}

/// As portas do turno. Hoje a API intermediária é o backend e o journal ao
/// mesmo tempo (`intermediary.rs`).
pub struct TurnPorts<'a> {
    pub backend: &'a dyn ModelBackend,
    pub journal: &'a dyn TurnJournal,
    pub db: &'a dyn TurnDatabase,
    pub structure: &'a dyn TurnStructure,
}

/// Tudo que o turno precisa, montado antes (comando `assistant_turn_ask`).
pub struct TurnSetup {
    pub conversation_id: String,
    /// Idempotência na API e id da execução no banco (para cancelar)
    pub client_turn_id: String,
    pub question: String,
    pub language: Detected,
    pub prompt: Prompt,
    pub db_type: DatabaseType,
    pub expected_language: QueryLanguage,
    pub schema: SchemaContext,
    pub history: Vec<HistoryTurn>,
    /// O modelo da conversa (a janela de contexto dos orçamentos)
    pub model: ModelProfile,
    pub auto_execute: bool,
    /// A opção de valores de exemplo do servidor, onde ela existe (o Mongo:
    /// `assistant_sample_values` das capabilities); `None` nos outros bancos
    pub sample_values: Option<bool>,
    /// Espera máxima pela resposta a uma pergunta do modelo ([`REPLY_TIMEOUT`])
    pub reply_timeout: Duration,
    /// Primeira página da execução: `limit`, `schema` do `search_path` e o
    /// `query_id` (= `client_turn_id`)
    pub page: QueryOptions,
}

/// Os turnos anteriores para o histórico das mensagens (C-12): a pergunta e a
/// resposta da última tentativa gerada de cada um, na ordem da conversa. O
/// pedido de estrutura é do meio do turno e não entra; a resposta em texto
/// entra (é a resposta que a pessoa viu).
pub fn history_from(conversation: &AssistantConversation) -> Vec<HistoryTurn> {
    conversation
        .turns
        .iter()
        .filter_map(|turn| {
            let output = turn
                .attempts
                .iter()
                .rev()
                .filter(|attempt| attempt.status == AssistantAttemptStatus::Generated)
                .filter_map(|attempt| attempt.output.clone())
                .find(|output| !matches!(output.kind, OutputKind::StructureRequest | OutputKind::Unknown))?;
            Some(HistoryTurn {
                question: turn.summary.question.clone(),
                output,
            })
        })
        .collect()
}

/// Como uma consulta (`kind = query`) terminou.
enum QueryEnd {
    Answered,
    /// Falhou por causa da consulta: vai para a correção
    Retry(Correction),
    Cancelled,
    /// Falhou por outro motivo (conexão…): o turno para
    Failed(String),
}

/// Roda o turno inteiro. `Err` só quando o turno nem abriu (sem termo, sem
/// cota…); depois disso, todo desfecho volta como [`AssistantTurnFinish`].
pub async fn run_turn(
    ports: &TurnPorts<'_>,
    setup: &TurnSetup,
    emit: &(dyn Fn(AssistantTurnEvent) + Send + Sync),
    cancel: &CancelFlag,
    replies: &ReplySlot,
) -> Result<AssistantTurnFinish> {
    let TurnPorts {
        backend,
        journal,
        db,
        structure,
    } = *ports;
    let turn = journal
        .open_turn(
            &setup.conversation_id,
            &setup.question,
            setup.language,
            &setup.client_turn_id,
        )
        .await?;
    emit(AssistantTurnEvent::TurnOpened {
        turn_id: turn.id.clone(),
    });
    emit(AssistantTurnEvent::Context {
        tables: setup.schema.tables as u32,
        truncated: setup.schema.truncated,
        missing_mentions: setup.schema.mentions.missing.clone(),
        shortened_mentions: setup.schema.mentions.shortened.clone(),
    });
    log::info!("assistente: turno {} aberto (modelo {})", turn.id, setup.model.id);

    let mut rounds: Vec<Round> = Vec::new();
    let mut last: Option<(String, ModelOutput)> = None;
    let mut attempts = 0;
    let mut structure_requests = 0;
    let mut user_questions = 0;
    // A resposta do usuário vai à API só na tentativa logo depois dela
    let mut user_reply: Option<String> = None;
    // A estrutura pedida tem um orçamento próprio por turno, do tamanho do
    // da estrutura da pergunta, dividido pelas rodadas
    let mut requested_tokens = 0;

    let (status, error_code) = loop {
        if cancel.is_cancelled() {
            break (AssistantTurnStatus::Cancelled, None);
        }
        if attempts >= MAX_ATTEMPTS {
            break (AssistantTurnStatus::Exhausted, None);
        }

        let mut request = attempt_request(
            setup.prompt,
            &setup.schema,
            &setup.history,
            &setup.question,
            &rounds,
            setup.model.context_window,
        );
        request.context.user_reply = user_reply.clone();
        request.context.sample_values = setup.sample_values;

        let reply = match generate(backend, &turn.id, &request, cancel, emit).await {
            Ok(reply) => {
                user_reply = None;
                reply
            }
            Err(Error::Cancelled) => break (AssistantTurnStatus::Cancelled, None),
            Err(error) if is_api(&error, AssistantCode::TurnLimitExceeded) => {
                break (AssistantTurnStatus::Exhausted, None)
            }
            Err(error) => break (AssistantTurnStatus::Failed, Some(error.code().to_string())),
        };
        attempts += 1;
        let attempt_id = reply.attempt_id.clone();
        if let Some(usage) = &reply.usage {
            log::info!(
                "assistente: tentativa {attempt_id} gerada ({} tokens de entrada, {} de saída)",
                usage.input_tokens + usage.cache_read_tokens + usage.cache_write_tokens,
                usage.output_tokens
            );
        }
        if !reply.recorded {
            // Registro do experimento: falhar ao gravar não derruba o turno
            if let Err(error) = journal.record_attempt(&turn.id, &request, &reply).await {
                log::warn!("assistente: tentativa {attempt_id} não foi gravada ({})", error.code());
            }
        }

        let value = match reply.result {
            Ok(value) => value,
            // A API já registrou a tentativa inválida: o modelo vê o que deu
            // e o motivo na próxima
            Err(error) if error.code == AssistantCode::InvalidOutput => {
                log::info!("assistente: tentativa {attempt_id} com saída inválida");
                rounds.push(invalid_round(reply.raw, reply.stop, setup.expected_language));
                continue;
            }
            Err(error) if error.code == AssistantCode::TurnLimitExceeded => {
                break (AssistantTurnStatus::Exhausted, None)
            }
            Err(error) => {
                break (AssistantTurnStatus::Failed, Some(error.code.app_code().to_string()))
            }
        };

        let output = match output::parse(value, setup.expected_language) {
            Ok(output) => output,
            Err(reason) => {
                // Fora do contrato pelo lado do app (a linguagem não é a do banco)
                log::info!("assistente: tentativa {attempt_id} fora do contrato no app");
                report(journal, &attempt_id, &not_run("invalid_output", setup.auto_execute)).await;
                rounds.push(Round::Invalid(InvalidRound {
                    raw: reply.raw,
                    reason: format!("The JSON does not follow the format: {reason}."),
                }));
                continue;
            }
        };

        if output.kind == OutputKind::StructureRequest {
            // Chamada utilitária: não vai para a tela como resposta
            let names = output.structure_request.clone().unwrap_or_default();
            let reply = if structure_requests < MAX_STRUCTURE_REQUESTS {
                structure_requests += 1;
                attempts -= 1;
                let budget = budget_tokens(setup.model.context_window).saturating_sub(requested_tokens);
                match structure.lookup(&names, budget).await {
                    Ok(found) => {
                        requested_tokens += found.tokens_estimate;
                        StructureReply::Found(found)
                    }
                    Err(error) => {
                        report(journal, &attempt_id, &not_run("kind_structure_request", setup.auto_execute)).await;
                        break (AssistantTurnStatus::Failed, Some(error.code().to_string()));
                    }
                }
            } else {
                StructureReply::LimitReached
            };
            let (outcome_reason, event) = match &reply {
                StructureReply::Found(found) => {
                    log::info!(
                        "assistente: tentativa {attempt_id} pediu estrutura: {} nome(s), {} encontrado(s), {} não encontrado(s){}",
                        found.requested,
                        found.found,
                        found.missing.len(),
                        if found.truncated { ", recortado" } else { "" }
                    );
                    (
                        "kind_structure_request",
                        AssistantTurnEvent::StructureLookup {
                            attempt_id: attempt_id.clone(),
                            requested: found.requested as u32,
                            found: found.found as u32,
                            missing: found.missing.clone(),
                            truncated: found.truncated,
                            limit_reached: false,
                        },
                    )
                }
                StructureReply::LimitReached => {
                    log::info!("assistente: tentativa {attempt_id} pediu estrutura além do teto do turno");
                    (
                        "structure_limit",
                        AssistantTurnEvent::StructureLookup {
                            attempt_id: attempt_id.clone(),
                            requested: names.len() as u32,
                            found: 0,
                            missing: Vec::new(),
                            truncated: false,
                            limit_reached: true,
                        },
                    )
                }
            };
            emit(event);
            report(journal, &attempt_id, &not_run(outcome_reason, setup.auto_execute)).await;
            rounds.push(Round::Structure(StructureRound { output, reply }));
            continue;
        }

        if output.kind == OutputKind::AskUser {
            let reply = if user_questions < MAX_USER_QUESTIONS {
                user_questions += 1;
                attempts -= 1;
                // A espera abre antes de a pergunta ir para a tela: uma
                // resposta rápida não se perde
                let waiting = replies.open();
                emit(AssistantTurnEvent::Question {
                    attempt_id: attempt_id.clone(),
                    question: output.clarification_question.clone().unwrap_or_default(),
                    choices: output.choices.clone().unwrap_or_default(),
                });
                report(journal, &attempt_id, &not_run("kind_ask_user", setup.auto_execute)).await;
                log::info!("assistente: tentativa {attempt_id} perguntou ao usuário; esperando a resposta");
                match wait_reply(waiting, cancel, setup.reply_timeout).await {
                    Waited::Reply(text) => {
                        emit(AssistantTurnEvent::UserReplied {
                            attempt_id: attempt_id.clone(),
                            reply: text.clone(),
                        });
                        user_reply = Some(text.clone());
                        QuestionReply::Answered(text)
                    }
                    Waited::Cancelled => {
                        replies.close();
                        break (AssistantTurnStatus::Cancelled, None);
                    }
                    Waited::Unanswered => {
                        // A pergunta fica como a resposta do turno
                        replies.close();
                        log::info!("assistente: tentativa {attempt_id} ficou sem resposta do usuário");
                        last = Some((attempt_id.clone(), output.clone()));
                        break (AssistantTurnStatus::Clarification, None);
                    }
                }
            } else {
                log::info!("assistente: tentativa {attempt_id} perguntou além do teto do turno");
                report(journal, &attempt_id, &not_run("user_question_limit", setup.auto_execute)).await;
                QuestionReply::LimitReached
            };
            rounds.push(Round::Question(QuestionRound { output, reply }));
            continue;
        }

        emit(AssistantTurnEvent::Output {
            attempt_id: attempt_id.clone(),
            output: output.clone(),
        });
        last = Some((attempt_id.clone(), output.clone()));

        match output.kind {
            OutputKind::Clarification => {
                report(journal, &attempt_id, &not_run("kind_clarification", setup.auto_execute)).await;
                break (AssistantTurnStatus::Clarification, None);
            }
            OutputKind::Code => {
                // Código nunca roda (RN-14): só vai para a tela
                report(journal, &attempt_id, &not_run("kind_code", setup.auto_execute)).await;
                break (AssistantTurnStatus::CodeDelivered, None);
            }
            OutputKind::Answer => {
                // Resposta em texto: nada a executar
                report(journal, &attempt_id, &not_run("kind_answer", setup.auto_execute)).await;
                break (AssistantTurnStatus::Replied, None);
            }
            // Tratados acima; o `parse` recusa o `Unknown`
            OutputKind::AskUser | OutputKind::StructureRequest | OutputKind::Unknown => {
                rounds.push(invalid_round(reply.raw, reply.stop, setup.expected_language));
                continue;
            }
            OutputKind::Query => {
                let query = output.query.clone().unwrap_or_default();
                match run_query(journal, db, setup, &attempt_id, &query, emit, cancel).await {
                    QueryEnd::Answered => break (AssistantTurnStatus::Answered, None),
                    QueryEnd::Cancelled => break (AssistantTurnStatus::Cancelled, None),
                    QueryEnd::Failed(code) => break (AssistantTurnStatus::Failed, Some(code)),
                    QueryEnd::Retry(mut correction) => {
                        correction.output = output;
                        rounds.push(Round::Correction(correction));
                    }
                }
            }
        }
    };

    if let Err(error) = journal.finish_turn(&turn.id, status).await {
        log::warn!("assistente: falha ao encerrar o turno {} ({})", turn.id, error.code());
    }
    log::info!(
        "assistente: turno {} encerrado ({status:?}, {attempts} tentativa(s), {structure_requests} pedido(s) de estrutura, {user_questions} pergunta(s) ao usuário)",
        turn.id
    );

    let (attempt_id, output) = last.unzip();
    Ok(AssistantTurnFinish {
        turn_id: Some(turn.id),
        status,
        attempt_id,
        output,
        error_code,
    })
}

/// A rodada de uma saída que o backend recusou: o app confere o texto de
/// novo para dizer ao modelo o que estava errado. Cortada pelo limite de
/// saída (o backend diz), o motivo é esse.
fn invalid_round(raw: String, stop: Option<StopReason>, expected: QueryLanguage) -> Round {
    let reason = match stop {
        Some(StopReason::MaxTokens) => output::OUTPUT_LIMIT_REASON.to_string(),
        _ => output::invalid_reason(&raw, expected),
    };
    Round::Invalid(InvalidRound { raw, reason })
}

/// Como terminou a espera pela resposta do usuário.
enum Waited {
    Reply(String),
    Cancelled,
    /// O tempo acabou (ou a espera foi fechada sem resposta)
    Unanswered,
}

async fn wait_reply(
    waiting: tokio::sync::oneshot::Receiver<String>,
    cancel: &CancelFlag,
    timeout: Duration,
) -> Waited {
    tokio::select! {
        reply = waiting => match reply {
            Ok(text) => Waited::Reply(text),
            Err(_) => Waited::Unanswered,
        },
        _ = cancel.cancelled() => Waited::Cancelled,
        _ = tokio::time::sleep(timeout) => Waited::Unanswered,
    }
}

/// Pede a tentativa, esperando e repetindo quando o backend pede calma (429).
async fn generate(
    backend: &dyn ModelBackend,
    turn_id: &str,
    request: &AttemptRequest,
    cancel: &CancelFlag,
    emit: &(dyn Fn(AssistantTurnEvent) + Send + Sync),
) -> Result<Generation> {
    let progress = |event: AttemptProgress| match event {
        AttemptProgress::Started { attempt_id, index } => {
            emit(AssistantTurnEvent::AttemptStarted { attempt_id, index })
        }
        AttemptProgress::Delta(text) => emit(AssistantTurnEvent::Delta { text }),
    };

    let mut waits = 0;
    loop {
        match backend.generate(turn_id, request, cancel, &progress).await {
            Err(Error::Assistant(error))
                if error.code == AssistantCode::RateLimited && waits < RATE_LIMIT_RETRIES =>
            {
                waits += 1;
                let wait = error.retry_after.unwrap_or(RATE_LIMIT_DEFAULT_WAIT);
                emit(AssistantTurnEvent::Waiting {
                    seconds: wait.as_secs().max(1) as u32,
                });
                tokio::select! {
                    _ = tokio::time::sleep(wait) => {}
                    _ = cancel.cancelled() => return Err(Error::Cancelled),
                }
            }
            other => return other,
        }
    }
}

/// Guard, preflight e (com a automática ligada) a execução da consulta.
async fn run_query(
    journal: &dyn TurnJournal,
    db: &dyn TurnDatabase,
    setup: &TurnSetup,
    attempt_id: &str,
    query: &str,
    emit: &(dyn Fn(AssistantTurnEvent) + Send + Sync),
    cancel: &CancelFlag,
) -> QueryEnd {
    let op = ReadOnlyOp::new(setup.expected_language, query);
    let mut outcome = AttemptOutcome {
        guard: GuardResult {
            allowed: true,
            reason: None,
        },
        preflight: StageResult::skipped(),
        execution: StageResult::skipped(),
        row_count: None,
        auto_execute: setup.auto_execute,
        user_edited: false,
    };

    // Guard
    match guard::inspect(&op, setup.db_type) {
        Ok(None) => {}
        Ok(Some(rejection)) => {
            let message = rejection.to_string();
            emit(stage_failed(attempt_id, AssistantStage::Guard, Some(FailureClass::GuardRejected), &message));
            outcome.guard = GuardResult {
                allowed: false,
                reason: Some(rejection.code().to_string()),
            };
            report(journal, attempt_id, &outcome).await;
            // A mensagem do guard é texto nosso, sem trecho da consulta
            return QueryEnd::Retry(correction(AssistantStage::Guard, FailureClass::GuardRejected, Some(message)));
        }
        Err(error) => return QueryEnd::Failed(error.code().to_string()),
    }

    // Preflight (EXPLAIN, sem executar)
    let started = Instant::now();
    let plan = db.preflight(&op, setup.page.clone()).await;
    let duration = Some(started.elapsed().as_millis() as i64);
    match plan {
        Ok(plan) => {
            outcome.preflight = passed(duration);
            emit(AssistantTurnEvent::Plan {
                attempt_id: attempt_id.to_string(),
                plan,
            });
        }
        Err(error) => {
            if cancel.is_cancelled() {
                return QueryEnd::Cancelled;
            }
            return stage_error(journal, attempt_id, &mut outcome, AssistantStage::Preflight, error, query, duration, emit).await;
        }
    }

    if !setup.auto_execute {
        // Revisão: a consulta e o plano ficam na tela; o usuário executa
        report(journal, attempt_id, &outcome).await;
        return QueryEnd::Answered;
    }

    // Execução de leitura, cancelável no meio
    let started = Instant::now();
    let execution = {
        let running = db.execute(&op, setup.page.clone());
        tokio::pin!(running);
        tokio::select! {
            result = &mut running => result,
            _ = cancel.cancelled() => {
                if let Some(query_id) = setup.page.query_id.as_deref() {
                    db.cancel(query_id).await;
                }
                let _ = tokio::time::timeout(CANCEL_GRACE, running).await;
                return QueryEnd::Cancelled;
            }
        }
    };
    let duration = Some(started.elapsed().as_millis() as i64);
    match execution {
        Ok(result) => {
            outcome.execution = passed(duration);
            outcome.row_count = Some(result.row_count as i64);
            report(journal, attempt_id, &outcome).await;
            emit(AssistantTurnEvent::Result {
                attempt_id: attempt_id.to_string(),
                result,
            });
            QueryEnd::Answered
        }
        Err(error) => {
            if cancel.is_cancelled() {
                return QueryEnd::Cancelled;
            }
            stage_error(journal, attempt_id, &mut outcome, AssistantStage::Execution, error, query, duration, emit).await
        }
    }
}

/// Falha numa etapa do banco: da consulta (vai para a correção, com o erro
/// sanitizado) ou não (conexão, banco fora: o turno para).
#[allow(clippy::too_many_arguments)]
async fn stage_error(
    journal: &dyn TurnJournal,
    attempt_id: &str,
    outcome: &mut AttemptOutcome,
    stage: AssistantStage,
    error: Error,
    query: &str,
    duration: Option<i64>,
    emit: &(dyn Fn(AssistantTurnEvent) + Send + Sync),
) -> QueryEnd {
    let class = classify(&error);
    let sanitized = sanitize(&error, query);
    // Na tela, o erro como o banco deu: fica na máquina do usuário
    emit(stage_failed(attempt_id, stage, class, &error.to_string()));

    let result = StageResult {
        ran: true,
        ok: Some(false),
        failure_class: class,
        error_sanitized: if class.is_some() { sanitized.clone() } else { None },
        duration_ms: duration,
    };
    match stage {
        AssistantStage::Execution => outcome.execution = result,
        _ => outcome.preflight = result,
    }
    report(journal, attempt_id, outcome).await;

    match class {
        Some(class) => QueryEnd::Retry(correction(stage, class, sanitized)),
        None => QueryEnd::Failed(error.code().to_string()),
    }
}

fn correction(stage: AssistantStage, class: FailureClass, error_sanitized: Option<String>) -> Correction {
    Correction {
        // Preenchida por quem chama, com a resposta da tentativa
        output: placeholder_output(),
        stage,
        class,
        error_sanitized,
    }
}

fn placeholder_output() -> ModelOutput {
    ModelOutput {
        kind: OutputKind::Query,
        language: None,
        query: None,
        explanation: String::new(),
        assumptions: Vec::new(),
        clarification_question: None,
        structure_request: None,
        choices: None,
    }
}

fn passed(duration: Option<i64>) -> StageResult {
    StageResult {
        ran: true,
        ok: Some(true),
        failure_class: None,
        error_sanitized: None,
        duration_ms: duration,
    }
}

/// Outcome de uma resposta que não é consulta (ou não vale como tal): nada
/// rodou, e o guard registra o porquê.
fn not_run(reason: &str, auto_execute: bool) -> AttemptOutcome {
    AttemptOutcome {
        guard: GuardResult {
            allowed: false,
            reason: Some(reason.to_string()),
        },
        preflight: StageResult::skipped(),
        execution: StageResult::skipped(),
        row_count: None,
        auto_execute,
        user_edited: false,
    }
}

fn stage_failed(
    attempt_id: &str,
    stage: AssistantStage,
    failure_class: Option<FailureClass>,
    message: &str,
) -> AssistantTurnEvent {
    AssistantTurnEvent::StageFailed {
        attempt_id: attempt_id.to_string(),
        stage,
        failure_class,
        message: message.to_string(),
    }
}

/// O outcome é registro do experimento: falhar ao mandar não derruba o turno.
async fn report(journal: &dyn TurnJournal, attempt_id: &str, outcome: &AttemptOutcome) {
    if let Err(error) = journal.report_outcome(attempt_id, outcome).await {
        log::warn!("assistente: outcome da tentativa {attempt_id} não foi registrado ({})", error.code());
    }
}

fn is_api(error: &Error, code: AssistantCode) -> bool {
    matches!(error, Error::Assistant(e) if e.code == code)
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::Arc;

    use parking_lot::Mutex;
    use serde_json::{json, Value};

    use super::*;
    use crate::assistant::error::AssistantError;
    use crate::assistant::prompts;
    use crate::models::{AssistantConversation, AssistantTurnStatus, AssistantTurnSummary, QueryColumnInfo};

    // ── API simulada ────────────────────────────────────────────────────────

    /// Cada pedido de tentativa consome a próxima resposta roteirizada.
    enum Scripted {
        Output(Value),
        StreamError(AssistantCode),
        /// `invalid_output` do backend, com o texto que o modelo deu e o
        /// motivo de parada que o backend informou
        Invalid(&'static str, Option<StopReason>),
        Before(AssistantError),
        /// Fica pendurado até o cancelamento
        Hang,
    }

    /// A API nas duas portas, como a intermediária. Com `unrecorded`, gera
    /// como um backend direto: a tentativa vai ao journal pelo turno.
    #[derive(Default)]
    struct FakeApi {
        script: Mutex<VecDeque<Scripted>>,
        requests: Mutex<Vec<AttemptRequest>>,
        outcomes: Mutex<Vec<(String, AttemptOutcome)>>,
        finished: Mutex<Option<AssistantTurnStatus>>,
        unrecorded: bool,
        /// O que chegou ao journal, na ordem: `record:<id>` e `outcome:<id>`
        journal: Mutex<Vec<String>>,
    }

    impl FakeApi {
        fn new(script: Vec<Scripted>) -> Self {
            Self {
                script: Mutex::new(script.into()),
                ..Default::default()
            }
        }
    }

    #[async_trait]
    impl TurnJournal for FakeApi {
        async fn conversation(&self, _: &str) -> Result<AssistantConversation> {
            unreachable!("o turno recebe a conversa pronta")
        }

        async fn open_turn(&self, _: &str, question: &str, language: Detected, _: &str) -> Result<AssistantTurnSummary> {
            Ok(AssistantTurnSummary {
                id: "turn-1".into(),
                index: 0,
                question: question.into(),
                language: language.language,
                status: AssistantTurnStatus::Open,
                created_at: "2026-10-20T00:00:00Z".into(),
            })
        }

        async fn record_attempt(&self, _: &str, _: &AttemptRequest, generation: &Generation) -> Result<()> {
            assert!(!generation.recorded, "a tentativa gravada pelo backend não volta ao journal");
            self.journal.lock().push(format!("record:{}", generation.attempt_id));
            Ok(())
        }

        async fn report_outcome(&self, attempt_id: &str, outcome: &AttemptOutcome) -> Result<()> {
            self.journal.lock().push(format!("outcome:{attempt_id}"));
            self.outcomes.lock().push((attempt_id.into(), outcome.clone()));
            Ok(())
        }

        async fn finish_turn(&self, _: &str, status: AssistantTurnStatus) -> Result<()> {
            *self.finished.lock() = Some(status);
            Ok(())
        }
    }

    #[async_trait]
    impl ModelBackend for FakeApi {
        async fn generate(
            &self,
            _: &str,
            request: &AttemptRequest,
            cancel: &CancelFlag,
            progress: &(dyn Fn(AttemptProgress) + Send + Sync),
        ) -> Result<Generation> {
            let index = self.requests.lock().len() as i64;
            self.requests.lock().push(request.clone());
            let next = self.script.lock().pop_front().expect("roteiro acabou");
            let attempt_id = format!("attempt-{index}");
            let started = |progress: &(dyn Fn(AttemptProgress) + Send + Sync)| {
                progress(AttemptProgress::Started {
                    attempt_id: attempt_id.clone(),
                    index,
                })
            };
            let generation = |result, raw: String, stop| Generation {
                attempt_id: attempt_id.clone(),
                index,
                result,
                raw,
                usage: None,
                stop,
                recorded: !self.unrecorded,
            };
            match next {
                Scripted::Output(value) => {
                    started(progress);
                    let raw = value.to_string();
                    Ok(generation(Ok(value), raw, Some(StopReason::End)))
                }
                Scripted::StreamError(code) => {
                    started(progress);
                    Ok(generation(Err(AssistantError::new(code, "x")), String::new(), None))
                }
                Scripted::Invalid(raw, stop) => {
                    started(progress);
                    let error = AssistantError::new(AssistantCode::InvalidOutput, "fora do contrato");
                    Ok(generation(Err(error), raw.into(), stop))
                }
                Scripted::Before(error) => Err(error.into()),
                Scripted::Hang => {
                    started(progress);
                    cancel.cancelled().await;
                    Err(Error::Cancelled)
                }
            }
        }
    }

    // ── Banco simulado ──────────────────────────────────────────────────────

    #[derive(Default)]
    struct FakeDb {
        /// Erros do preflight, em ordem (vazio = passa)
        preflight_errors: Mutex<VecDeque<Error>>,
        execute_errors: Mutex<VecDeque<Error>>,
        /// A execução fica pendurada até o cancelamento
        hang: bool,
        executed: Mutex<Vec<String>>,
        languages: Mutex<Vec<QueryLanguage>>,
        cancelled: Mutex<Vec<String>>,
        released: tokio::sync::Notify,
    }

    #[async_trait]
    impl TurnDatabase for FakeDb {
        async fn preflight(&self, _: &ReadOnlyOp, _: QueryOptions) -> Result<ReadOnlyPlan> {
            match self.preflight_errors.lock().pop_front() {
                Some(error) => Err(error),
                None => Ok(ReadOnlyPlan {
                    estimated_rows: Some(10.0),
                    estimated_cost: Some(1.0),
                    plan: "[]".into(),
                }),
            }
        }

        async fn execute(&self, op: &ReadOnlyOp, _: QueryOptions) -> Result<QueryResult> {
            let (ReadOnlyOp::Sql(text) | ReadOnlyOp::Mongo(text) | ReadOnlyOp::RedisPlan(text)) = op;
            self.executed.lock().push(text.clone());
            self.languages.lock().push(op.language());
            if self.hang {
                self.released.notified().await;
                return Err(Error::Query("canceling statement due to user request".into()));
            }
            if let Some(error) = self.execute_errors.lock().pop_front() {
                return Err(error);
            }
            Ok(QueryResult {
                columns: vec![QueryColumnInfo {
                    name: "total".into(),
                    type_name: "int8".into(),
                    type_oid: Some(20),
                }],
                rows: vec![vec![Some("3".into())]],
                row_count: 1,
                total_count: None,
                has_more: false,
                execution_time_ms: 1,
                editable_info: None,
            })
        }

        async fn cancel(&self, query_id: &str) {
            self.cancelled.lock().push(query_id.into());
            self.released.notify_waiters();
        }
    }

    // ── Roteiro ─────────────────────────────────────────────────────────────

    fn output(kind: &str, sql: Option<&str>) -> Value {
        json!({
            "kind": kind,
            "language": sql.map(|_| "sql"),
            "query": sql,
            "explanation": "Conta os pedidos",
            "assumptions": [],
            "clarificationQuestion": if kind == "clarification" { json!("Qual tabela?") } else { Value::Null },
            "structureRequest": null,
            "choices": null,
        })
    }

    fn structure_request(names: &[&str]) -> Value {
        json!({
            "kind": "structure_request", "language": null, "query": null, "explanation": "",
            "assumptions": [], "clarificationQuestion": null, "structureRequest": names,
            "choices": null,
        })
    }

    fn text_answer(explanation: &str) -> Value {
        json!({
            "kind": "answer", "language": null, "query": null, "explanation": explanation,
            "assumptions": [], "clarificationQuestion": null, "structureRequest": null,
            "choices": null,
        })
    }

    fn ask_user(question: &str) -> Value {
        json!({
            "kind": "ask_user", "language": null, "query": null, "explanation": "",
            "assumptions": [], "clarificationQuestion": question, "structureRequest": null,
            "choices": ["Do pedido", "Do pagamento"],
        })
    }

    // ── Estrutura simulada ──────────────────────────────────────────────────

    /// Acha os nomes que começam com `sales.`; guarda cada pedido e o
    /// orçamento que veio.
    #[derive(Default)]
    struct FakeStructure {
        lookups: Mutex<Vec<(Vec<String>, usize)>>,
        fail: bool,
    }

    #[async_trait]
    impl TurnStructure for FakeStructure {
        async fn lookup(&self, names: &[String], budget_tokens: usize) -> Result<RequestedStructure> {
            self.lookups.lock().push((names.to_vec(), budget_tokens));
            if self.fail {
                return Err(Error::Connection("catalog lane closed".into()));
            }
            let (found, missing): (Vec<&String>, Vec<&String>) = names.iter().partition(|name| name.starts_with("sales."));
            let text = format!(
                "CREATE TABLE {};\n-- Not found: {}",
                found.iter().map(|name| name.as_str()).collect::<Vec<_>>().join(", "),
                missing.iter().map(|name| name.as_str()).collect::<Vec<_>>().join(", ")
            );
            Ok(RequestedStructure {
                tokens_estimate: 1_000,
                text,
                requested: names.len(),
                found: found.len(),
                missing: missing.into_iter().cloned().collect(),
                truncated: false,
            })
        }
    }

    fn setup(auto_execute: bool) -> TurnSetup {
        TurnSetup {
            conversation_id: "conv-1".into(),
            client_turn_id: "client-1".into(),
            question: "quantos pedidos?".into(),
            language: Detected {
                language: crate::assistant::language::Language::Pt,
                confidence: 0.9,
            },
            prompt: prompts::for_db(DatabaseType::Postgres).unwrap(),
            db_type: DatabaseType::Postgres,
            expected_language: QueryLanguage::Sql,
            schema: SchemaContext {
                ddl: "-- PostgreSQL".into(),
                tables: 1,
                truncated: false,
                tokens_estimate: 4,
                mentions: Default::default(),
            },
            history: vec![],
            model: ModelProfile {
                id: "model-1".into(),
                context_window: 200_000,
            },
            auto_execute,
            sample_values: None,
            reply_timeout: Duration::from_secs(5),
            page: QueryOptions {
                limit: 200,
                query_id: Some("client-1".into()),
                ..QueryOptions::default()
            },
        }
    }

    struct Run {
        finish: AssistantTurnFinish,
        events: Vec<AssistantTurnEvent>,
    }

    async fn run(api: &FakeApi, db: &FakeDb, setup: &TurnSetup, cancel: &CancelFlag) -> Run {
        run_with(api, db, &FakeStructure::default(), setup, cancel).await
    }

    async fn run_with(api: &FakeApi, db: &FakeDb, structure: &FakeStructure, setup: &TurnSetup, cancel: &CancelFlag) -> Run {
        let events: Arc<Mutex<Vec<AssistantTurnEvent>>> = Arc::default();
        let sink = Arc::clone(&events);
        let ports = TurnPorts {
            backend: api,
            journal: api,
            db,
            structure,
        };
        let finish = run_turn(&ports, setup, &move |event| sink.lock().push(event), cancel, &ReplySlot::default())
            .await
            .unwrap();
        let events = events.lock().clone();
        Run { finish, events }
    }

    /// Como a tela: a cada pergunta do modelo, responde com a próxima de
    /// `answers` (acabou = não responde).
    async fn run_replying(api: &FakeApi, db: &FakeDb, setup: &TurnSetup, cancel: &CancelFlag, answers: &[&str]) -> Run {
        let events: Arc<Mutex<Vec<AssistantTurnEvent>>> = Arc::default();
        let sink = Arc::clone(&events);
        let replies = ReplySlot::default();
        let screen = replies.clone();
        let answers = Mutex::new(answers.iter().map(|answer| answer.to_string()).collect::<VecDeque<_>>());
        let emit = move |event: AssistantTurnEvent| {
            if matches!(event, AssistantTurnEvent::Question { .. }) {
                if let Some(answer) = answers.lock().pop_front() {
                    assert!(screen.reply(answer), "a espera abre antes da pergunta ir para a tela");
                }
            }
            sink.lock().push(event);
        };
        let ports = TurnPorts {
            backend: api,
            journal: api,
            db,
            structure: &FakeStructure::default(),
        };
        let finish = run_turn(&ports, setup, &emit, cancel, &replies).await.unwrap();
        let events = events.lock().clone();
        Run { finish, events }
    }

    /// O turno numa task, para o teste cancelar ou responder no meio.
    fn spawn_turn(
        api: &Arc<FakeApi>,
        db: &Arc<FakeDb>,
        setup: TurnSetup,
        cancel: &CancelFlag,
        replies: &ReplySlot,
    ) -> tokio::task::JoinHandle<Result<AssistantTurnFinish>> {
        let (api, db, cancel, replies) = (Arc::clone(api), Arc::clone(db), cancel.clone(), replies.clone());
        tokio::spawn(async move {
            let structure = FakeStructure::default();
            let ports = TurnPorts {
                backend: &*api,
                journal: &*api,
                db: &*db,
                structure: &structure,
            };
            run_turn(&ports, &setup, &|_| {}, &cancel, &replies).await
        })
    }

    fn pg_error(sql_state: &str, message: &str) -> Error {
        Error::DbQuery {
            sql_state: sql_state.into(),
            message: message.into(),
        }
    }

    // ── Casos ───────────────────────────────────────────────────────────────

    /// Mongo: a operação vai como `ReadOnlyOp::Mongo`; o guard recusa o
    /// `$out` e a correção leva a mensagem dele ao modelo.
    #[tokio::test]
    async fn mongo_turn_guards_then_runs_the_operation() {
        let mongo = |query: &str| {
            let mut value = output("query", Some(query));
            value["language"] = json!("mongo");
            value
        };
        let write = r#"{"collection":"orders","operation":"aggregate","pipeline":[{"$out":"copy"}]}"#;
        let read = r#"{"collection":"orders","operation":"countDocuments","filter":{}}"#;
        let api = FakeApi::new(vec![
            // SQL numa conversa de Mongo: fora do contrato no app
            Scripted::Output(output("query", Some("SELECT count(*) FROM orders"))),
            Scripted::Output(mongo(write)),
            Scripted::Output(mongo(read)),
        ]);
        let db = FakeDb::default();
        let mut setup = setup(true);
        setup.prompt = prompts::for_db(DatabaseType::Mongodb).unwrap();
        setup.db_type = DatabaseType::Mongodb;
        setup.expected_language = QueryLanguage::Mongo;
        setup.sample_values = Some(true);

        let run = run(&api, &db, &setup, &CancelFlag::default()).await;

        assert_eq!(run.finish.status, AssistantTurnStatus::Answered);
        assert_eq!(*db.executed.lock(), [read]);
        assert_eq!(*db.languages.lock(), [QueryLanguage::Mongo]);
        let outcomes = api.outcomes.lock();
        assert_eq!(outcomes[0].1.guard.reason.as_deref(), Some("invalid_output"));
        assert_eq!(outcomes[1].1.guard.reason.as_deref(), Some("write_stage"));
        assert!(outcomes[2].1.execution.ok == Some(true));
        let requests = api.requests.lock();
        assert_eq!(requests[0].prompt_version, "mongo-v5");
        // A opção de valores de exemplo vai em toda tentativa, para a análise
        assert!(requests.iter().all(|request| request.context.sample_values == Some(true)));
        let correction = &requests[2].messages.last().unwrap().content;
        assert!(
            correction.contains("class=\"guard_rejected\">\n$out writes to the database and is not allowed"),
            "{correction}"
        );
    }

    /// Redis: o plano vai como `ReadOnlyOp::RedisPlan`; o guard recusa o
    /// `KEYS` e a correção leva a mensagem dele ao modelo.
    #[tokio::test]
    async fn redis_turn_guards_then_runs_the_plan() {
        let redis = |query: &str| {
            let mut value = output("query", Some(query));
            value["language"] = json!("redis_plan");
            value
        };
        let keys = r#"{"steps":[{"id":"all","command":"KEYS","args":["user:*"],"foreach":null}],"output":"all"}"#;
        let scan = r#"{"steps":[{"id":"all","command":"SCAN","args":["0","MATCH","user:*"],"foreach":null}],"output":"all"}"#;
        let api = FakeApi::new(vec![Scripted::Output(redis(keys)), Scripted::Output(redis(scan))]);
        let db = FakeDb::default();
        let mut setup = setup(true);
        setup.prompt = prompts::for_db(DatabaseType::Redis).unwrap();
        setup.db_type = DatabaseType::Redis;
        setup.expected_language = QueryLanguage::RedisPlan;

        let run = run(&api, &db, &setup, &CancelFlag::default()).await;

        assert_eq!(run.finish.status, AssistantTurnStatus::Answered);
        assert_eq!(*db.executed.lock(), [scan]);
        assert_eq!(*db.languages.lock(), [QueryLanguage::RedisPlan]);
        let outcomes = api.outcomes.lock();
        assert_eq!(outcomes[0].1.guard.reason.as_deref(), Some("keys"));
        assert!(outcomes[1].1.execution.ok == Some(true));
        let requests = api.requests.lock();
        assert_eq!(requests[0].prompt_version, "redis-v3");
        let correction = &requests[1].messages.last().unwrap().content;
        assert!(
            correction.contains("class=\"guard_rejected\">\nKEYS is not allowed: use SCAN with MATCH"),
            "{correction}"
        );
    }

    #[tokio::test]
    async fn answers_and_executes_with_auto_execute() {
        let api = FakeApi::new(vec![Scripted::Output(output("query", Some("SELECT count(*) FROM orders")))]);
        let db = FakeDb::default();

        let run = run(&api, &db, &setup(true), &CancelFlag::default()).await;

        assert_eq!(run.finish.status, AssistantTurnStatus::Answered);
        assert_eq!(run.finish.attempt_id.as_deref(), Some("attempt-0"));
        assert_eq!(*db.executed.lock(), ["SELECT count(*) FROM orders"]);
        assert!(run.events.iter().any(|e| matches!(e, AssistantTurnEvent::Result { .. })));
        let outcomes = api.outcomes.lock();
        assert_eq!(outcomes.len(), 1);
        let outcome = &outcomes[0].1;
        assert!(outcome.guard.allowed && outcome.preflight.ok == Some(true) && outcome.execution.ok == Some(true));
        assert_eq!(outcome.row_count, Some(1));
        assert_eq!(*api.finished.lock(), Some(AssistantTurnStatus::Answered));
    }

    #[tokio::test]
    async fn review_mode_plans_but_never_executes() {
        let api = FakeApi::new(vec![Scripted::Output(output("query", Some("SELECT 1")))]);
        let db = FakeDb::default();

        let run = run(&api, &db, &setup(false), &CancelFlag::default()).await;

        assert_eq!(run.finish.status, AssistantTurnStatus::Answered);
        assert!(db.executed.lock().is_empty());
        assert!(run.events.iter().any(|e| matches!(e, AssistantTurnEvent::Plan { .. })));
        let outcome = &api.outcomes.lock()[0].1;
        assert!(!outcome.execution.ran && outcome.execution.ok.is_none());
        assert!(!outcome.auto_execute);
    }

    #[tokio::test]
    async fn code_is_never_executed() {
        let api = FakeApi::new(vec![Scripted::Output(output("code", Some("DELETE FROM orders")))]);
        let db = FakeDb::default();

        let run = run(&api, &db, &setup(true), &CancelFlag::default()).await;

        assert_eq!(run.finish.status, AssistantTurnStatus::CodeDelivered);
        assert_eq!(run.finish.output.unwrap().query.as_deref(), Some("DELETE FROM orders"));
        assert!(db.executed.lock().is_empty());
        let outcome = &api.outcomes.lock()[0].1;
        assert_eq!(outcome.guard, GuardResult { allowed: false, reason: Some("kind_code".into()) });
        assert!(!outcome.preflight.ran && !outcome.execution.ran);
    }

    #[tokio::test]
    async fn clarification_ends_the_turn() {
        let api = FakeApi::new(vec![Scripted::Output(output("clarification", None))]);
        let db = FakeDb::default();

        let run = run(&api, &db, &setup(true), &CancelFlag::default()).await;

        assert_eq!(run.finish.status, AssistantTurnStatus::Clarification);
        assert_eq!(api.requests.lock().len(), 1);
        assert!(db.executed.lock().is_empty());
    }

    /// A resposta em texto encerra o turno sem tocar no banco.
    #[tokio::test]
    async fn text_answer_ends_the_turn_as_replied() {
        let api = FakeApi::new(vec![Scripted::Output(text_answer("Um índice B-tree acelera igualdade e intervalo."))]);
        let db = FakeDb::default();

        let run = run(&api, &db, &setup(true), &CancelFlag::default()).await;

        assert_eq!(run.finish.status, AssistantTurnStatus::Replied);
        assert_eq!(run.finish.output.unwrap().kind, OutputKind::Answer);
        assert!(db.executed.lock().is_empty());
        let outcome = &api.outcomes.lock()[0].1;
        assert_eq!(outcome.guard, GuardResult { allowed: false, reason: Some("kind_answer".into()) });
        assert!(!outcome.preflight.ran && !outcome.execution.ran);
        assert_eq!(*api.finished.lock(), Some(AssistantTurnStatus::Replied));
    }

    /// O pedido de estrutura não conta como tentativa: o app busca os nomes,
    /// responde ao modelo e pergunta de novo; a tela recebe as contagens, e
    /// não o pedido como resposta.
    #[tokio::test]
    async fn structure_request_looks_up_and_asks_again() {
        let api = FakeApi::new(vec![
            Scripted::Output(structure_request(&["sales.orders", "ghost"])),
            Scripted::Output(output("query", Some("SELECT count(*) FROM sales.orders"))),
        ]);
        let db = FakeDb::default();
        let structure = FakeStructure::default();

        let run = run_with(&api, &db, &structure, &setup(true), &CancelFlag::default()).await;

        assert_eq!(run.finish.status, AssistantTurnStatus::Answered);
        assert_eq!(run.finish.attempt_id.as_deref(), Some("attempt-1"));
        assert_eq!(*db.executed.lock(), ["SELECT count(*) FROM sales.orders"]);
        let lookups = structure.lookups.lock();
        assert_eq!(lookups.len(), 1);
        assert_eq!(lookups[0].0, ["sales.orders", "ghost"]);
        assert_eq!(lookups[0].1, budget_tokens(200_000));

        let lookup = run
            .events
            .iter()
            .find_map(|event| match event {
                AssistantTurnEvent::StructureLookup { attempt_id, requested, found, missing, limit_reached, .. } => {
                    Some((attempt_id.clone(), *requested, *found, missing.clone(), *limit_reached))
                }
                _ => None,
            })
            .unwrap();
        assert_eq!(lookup, ("attempt-0".into(), 2, 1, vec!["ghost".to_string()], false));
        // O pedido não vai para a tela como resposta
        let outputs: Vec<&str> = run
            .events
            .iter()
            .filter_map(|event| match event {
                AssistantTurnEvent::Output { attempt_id, .. } => Some(attempt_id.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(outputs, ["attempt-1"]);

        let requests = api.requests.lock();
        let second = &requests[1];
        assert_eq!(second.messages.len(), 4);
        assert!(second.messages[2].content.contains("\"structureRequest\":[\"sales.orders\",\"ghost\"]"));
        assert!(second.messages[3].content.starts_with("<requested_structure>\nCREATE TABLE sales.orders;\n-- Not found: ghost"));
        assert_eq!(second.context.schema_tokens_estimate, 4 + 1_000);
        assert_eq!(second.context.previous_failure, None);
        let outcomes = api.outcomes.lock();
        assert_eq!(outcomes[0].1.guard.reason.as_deref(), Some("kind_structure_request"));
    }

    /// Até o teto, os pedidos não contam; o seguinte não busca nada, conta
    /// como tentativa e avisa o modelo. O orçamento é dividido pelas rodadas.
    #[tokio::test]
    async fn structure_requests_stop_at_the_ceiling() {
        let mut script: Vec<Scripted> = (0..MAX_STRUCTURE_REQUESTS + 1)
            .map(|i| Scripted::Output(structure_request(&[&format!("sales.t{i}")])))
            .collect();
        script.push(Scripted::Output(output("query", Some("SELECT 1"))));
        let api = FakeApi::new(script);
        let structure = FakeStructure::default();

        let run = run_with(&api, &FakeDb::default(), &structure, &setup(true), &CancelFlag::default()).await;

        assert_eq!(run.finish.status, AssistantTurnStatus::Answered);
        let lookups = structure.lookups.lock();
        assert_eq!(lookups.len(), MAX_STRUCTURE_REQUESTS);
        let budget = budget_tokens(200_000);
        let budgets: Vec<usize> = lookups.iter().map(|(_, budget)| *budget).collect();
        assert_eq!(budgets, [budget, budget - 1_000, budget - 2_000]);
        let requests = api.requests.lock();
        let last = requests.last().unwrap().messages.last().unwrap();
        assert!(last.content.starts_with("<requested_structure limit_reached=\"true\">"), "{}", last.content);
        assert!(run.events.iter().any(|event| matches!(event, AssistantTurnEvent::StructureLookup { limit_reached: true, .. })));
        assert_eq!(api.outcomes.lock()[MAX_STRUCTURE_REQUESTS].1.guard.reason.as_deref(), Some("structure_limit"));
    }

    /// Pedido de estrutura em laço esgota o turno pelo teto do app (sem a API
    /// cortar antes): os além do teto contam como tentativa.
    #[tokio::test]
    async fn endless_structure_requests_exhaust_the_turn() {
        let script = (0..MAX_STRUCTURE_REQUESTS + MAX_ATTEMPTS)
            .map(|_| Scripted::Output(structure_request(&["sales.orders"])))
            .collect();
        let api = FakeApi::new(script);

        let run = run(&api, &FakeDb::default(), &setup(true), &CancelFlag::default()).await;

        assert_eq!(run.finish.status, AssistantTurnStatus::Exhausted);
        assert_eq!(api.requests.lock().len(), MAX_STRUCTURE_REQUESTS + MAX_ATTEMPTS);
        assert!(run.finish.output.is_none());
    }

    /// A pergunta ao usuário pausa o turno: a resposta volta ao modelo (e à
    /// API, só na tentativa logo depois dela) e não conta como tentativa.
    #[tokio::test]
    async fn ask_user_waits_for_the_reply_and_continues() {
        let api = FakeApi::new(vec![
            Scripted::Output(ask_user("Pela data do pedido ou do pagamento?")),
            Scripted::StreamError(AssistantCode::InvalidOutput),
            Scripted::Output(output("query", Some("SELECT count(*) FROM orders"))),
        ]);

        let run = run_replying(&api, &FakeDb::default(), &setup(true), &CancelFlag::default(), &["Do pagamento"]).await;

        assert_eq!(run.finish.status, AssistantTurnStatus::Answered);
        assert_eq!(run.finish.output.unwrap().kind, OutputKind::Query);
        let question = run.events.iter().find_map(|event| match event {
            AssistantTurnEvent::Question { attempt_id, question, choices } => Some((attempt_id, question, choices)),
            _ => None,
        });
        let (attempt_id, question, choices) = question.expect("a pergunta foi para a tela");
        assert_eq!((attempt_id.as_str(), question.as_str()), ("attempt-0", "Pela data do pedido ou do pagamento?"));
        assert_eq!(choices, &["Do pedido", "Do pagamento"]);
        assert!(run.events.iter().any(|event| matches!(event,
            AssistantTurnEvent::UserReplied { attempt_id, reply } if attempt_id == "attempt-0" && reply == "Do pagamento")));
        // A pergunta não é a resposta do turno
        assert!(!run.events.iter().any(|event| matches!(event, AssistantTurnEvent::Output { attempt_id, .. } if attempt_id == "attempt-0")));

        let requests = api.requests.lock();
        let user_replies: Vec<Option<&str>> = requests.iter().map(|request| request.context.user_reply.as_deref()).collect();
        assert_eq!(user_replies, [None, Some("Do pagamento"), None]);
        assert!(requests[1].messages.last().unwrap().content.starts_with("<user_reply>\nDo pagamento\n</user_reply>"));
        assert!(requests[2].messages.iter().any(|message| message.content.starts_with("<user_reply>")));
        assert_eq!(api.outcomes.lock()[0].1.guard.reason.as_deref(), Some("kind_ask_user"));
    }

    /// Sem resposta no tempo, o turno termina como esclarecimento, com a
    /// pergunta como resposta.
    #[tokio::test]
    async fn unanswered_question_ends_as_clarification() {
        let api = FakeApi::new(vec![Scripted::Output(ask_user("Qual status conta como ativo?"))]);
        let mut setup = setup(true);
        setup.reply_timeout = Duration::from_millis(20);

        let run = run_replying(&api, &FakeDb::default(), &setup, &CancelFlag::default(), &[]).await;

        assert_eq!(run.finish.status, AssistantTurnStatus::Clarification);
        let output = run.finish.output.unwrap();
        assert_eq!(output.kind, OutputKind::AskUser);
        assert_eq!(output.clarification_question.as_deref(), Some("Qual status conta como ativo?"));
        assert_eq!(*api.finished.lock(), Some(AssistantTurnStatus::Clarification));
    }

    #[tokio::test]
    async fn cancel_while_waiting_for_the_reply() {
        let api = Arc::new(FakeApi::new(vec![Scripted::Output(ask_user("Qual data?"))]));
        let db = Arc::new(FakeDb::default());
        let cancel = CancelFlag::default();
        let replies = ReplySlot::default();
        let setup = setup(true);

        let running = spawn_turn(&api, &db, setup, &cancel, &replies);
        tokio::time::sleep(Duration::from_millis(20)).await;
        cancel.cancel();
        let finish = tokio::time::timeout(Duration::from_secs(2), running).await.unwrap().unwrap().unwrap();

        assert_eq!(finish.status, AssistantTurnStatus::Cancelled);
        assert!(!replies.reply("tarde".into()), "a espera fecha com o cancelamento");
    }

    /// Além do teto, a pergunta não vai para a tela: conta como tentativa e o
    /// modelo é avisado de que tem que responder com o que tem.
    #[tokio::test]
    async fn user_questions_stop_at_the_ceiling() {
        let mut script: Vec<Scripted> = (0..MAX_USER_QUESTIONS + 1)
            .map(|i| Scripted::Output(ask_user(&format!("Pergunta {i}?"))))
            .collect();
        script.push(Scripted::Output(output("query", Some("SELECT 1"))));
        let api = FakeApi::new(script);
        let answers: Vec<String> = (0..MAX_USER_QUESTIONS).map(|i| format!("resposta {i}")).collect();
        let answers: Vec<&str> = answers.iter().map(String::as_str).collect();

        let run = run_replying(&api, &FakeDb::default(), &setup(true), &CancelFlag::default(), &answers).await;

        assert_eq!(run.finish.status, AssistantTurnStatus::Answered);
        let shown = run.events.iter().filter(|event| matches!(event, AssistantTurnEvent::Question { .. })).count();
        assert_eq!(shown, MAX_USER_QUESTIONS);
        let requests = api.requests.lock();
        let last = requests.last().unwrap().messages.last().unwrap();
        assert!(last.content.starts_with("<user_reply limit_reached=\"true\">"), "{}", last.content);
        assert_eq!(api.outcomes.lock()[MAX_USER_QUESTIONS].1.guard.reason.as_deref(), Some("user_question_limit"));
    }

    #[tokio::test]
    async fn structure_lookup_failure_fails_the_turn() {
        let api = FakeApi::new(vec![Scripted::Output(structure_request(&["sales.orders"]))]);
        let structure = FakeStructure {
            fail: true,
            ..FakeStructure::default()
        };

        let run = run_with(&api, &FakeDb::default(), &structure, &setup(true), &CancelFlag::default()).await;

        assert_eq!(run.finish.status, AssistantTurnStatus::Failed);
        assert_eq!(run.finish.error_code.as_deref(), Some("CONNECTION_ERROR"));
    }

    #[tokio::test]
    async fn corrects_after_preflight_and_execution_errors() {
        let api = FakeApi::new(vec![
            Scripted::Output(output("query", Some("SELECT nome FROM orders"))),
            Scripted::Output(output("query", Some("SELECT name::int FROM orders"))),
            Scripted::Output(output("query", Some("SELECT name FROM orders"))),
        ]);
        let db = FakeDb::default();
        db.preflight_errors.lock().push_back(pg_error("42703", "column \"nome\" does not exist"));
        db.execute_errors
            .lock()
            .push_back(pg_error("22P02", "invalid input syntax for type integer: \"CANARY_7f3a\""));

        let run = run(&api, &db, &setup(true), &CancelFlag::default()).await;

        assert_eq!(run.finish.status, AssistantTurnStatus::Answered);
        let requests = api.requests.lock();
        assert_eq!(requests.len(), 3);
        assert_eq!(requests[0].context.previous_failure, None);
        assert_eq!(requests[1].context.previous_failure, Some(FailureClass::UnknownObject));
        assert_eq!(requests[2].context.previous_failure, Some(FailureClass::TypeMismatch));
        // A correção leva a resposta que falhou e o erro sanitizado, nunca o valor
        let last = requests[2].messages.iter().map(|m| m.content.as_str()).collect::<Vec<_>>().join("\n");
        assert!(last.contains("SELECT nome FROM orders") && last.contains("SELECT name::int FROM orders"));
        assert!(last.contains("42703: column \"nome\" does not exist"));
        assert!(!last.contains("CANARY_7f3a"), "{last}");
        // Na tela, o erro como o banco deu
        assert!(run.events.iter().any(|e| matches!(
            e,
            AssistantTurnEvent::StageFailed { stage: AssistantStage::Execution, message, .. } if message.contains("CANARY_7f3a")
        )));

        let outcomes = api.outcomes.lock();
        assert_eq!(outcomes.len(), 3);
        assert_eq!(outcomes[0].1.preflight.failure_class, Some(FailureClass::UnknownObject));
        assert!(!outcomes[0].1.execution.ran);
        let execution = &outcomes[1].1.execution;
        assert_eq!(execution.failure_class, Some(FailureClass::TypeMismatch));
        assert!(!execution.error_sanitized.as_deref().unwrap().contains("CANARY"));
    }

    #[tokio::test]
    async fn guard_rejection_goes_to_correction_without_touching_the_database() {
        let api = FakeApi::new(vec![
            Scripted::Output(output("query", Some("SELECT 1; DELETE FROM orders"))),
            Scripted::Output(output("query", Some("SELECT 1"))),
        ]);
        let db = FakeDb::default();

        let run = run(&api, &db, &setup(true), &CancelFlag::default()).await;

        assert_eq!(run.finish.status, AssistantTurnStatus::Answered);
        assert_eq!(*db.executed.lock(), ["SELECT 1"]);
        let outcome = &api.outcomes.lock()[0].1;
        assert_eq!(outcome.guard.reason.as_deref(), Some("multiple_statements"));
        assert_eq!(api.requests.lock()[1].context.previous_failure, Some(FailureClass::GuardRejected));
    }

    #[tokio::test]
    async fn exhausted_delivers_the_last_query() {
        let api = FakeApi::new(vec![
            Scripted::Output(output("query", Some("SELECT nome FROM a"))),
            Scripted::Output(output("query", Some("SELECT nome FROM b"))),
            Scripted::Before(AssistantError::new(AssistantCode::TurnLimitExceeded, "limit")),
        ]);
        let db = FakeDb::default();
        for _ in 0..2 {
            db.preflight_errors.lock().push_back(pg_error("42703", "column \"nome\" does not exist"));
        }

        let run = run(&api, &db, &setup(true), &CancelFlag::default()).await;

        assert_eq!(run.finish.status, AssistantTurnStatus::Exhausted);
        assert_eq!(run.finish.output.unwrap().query.as_deref(), Some("SELECT nome FROM b"));
        assert_eq!(*api.finished.lock(), Some(AssistantTurnStatus::Exhausted));
        assert!(db.executed.lock().is_empty());
    }

    /// A saída inválida volta ao modelo com o motivo: sem isso, a tentativa
    /// seguinte recebia as mesmas mensagens e, com temperatura 0, repetia o
    /// erro até o limite.
    #[tokio::test]
    async fn invalid_output_goes_back_to_the_model_with_the_reason() {
        let mut wrong_language = output("query", Some("{}"));
        wrong_language["language"] = json!("mongo");
        let api = FakeApi::new(vec![
            // Cortada no limite de saída: a API recusa
            Scripted::Invalid(r#"{"kind":"query","language":"sql","query":"SELECT"#, None),
            // Linguagem errada para o banco: inválida no app
            Scripted::Output(wrong_language.clone()),
            Scripted::Output(output("query", Some("SELECT 1"))),
        ]);
        let db = FakeDb::default();

        let run = run(&api, &db, &setup(true), &CancelFlag::default()).await;

        assert_eq!(run.finish.status, AssistantTurnStatus::Answered);
        let requests = api.requests.lock();
        assert_eq!(requests[0].messages.len(), 2);
        assert_eq!(requests[1].context.previous_failure, Some(FailureClass::InvalidOutput));
        assert_eq!(requests[2].context.previous_failure, Some(FailureClass::InvalidOutput));
        // A segunda vê a primeira como veio e o motivo
        let second = &requests[1].messages;
        assert_eq!(second.len(), 4);
        assert_eq!(second[2].content, r#"{"kind":"query","language":"sql","query":"SELECT"#);
        assert!(second[3].content.contains("stage=\"output\" class=\"invalid_output\">\nThe JSON ended before it was complete"), "{}", second[3].content);
        // A terceira vê as duas, cada uma com o seu motivo
        let third = &requests[2].messages;
        assert_eq!(third.len(), 6);
        assert_eq!(third[4].content, wrong_language.to_string());
        assert!(third[5].content.contains("language Mongo does not match the database (Sql)"), "{}", third[5].content);
        assert_eq!(api.outcomes.lock()[0].1.guard.reason.as_deref(), Some("invalid_output"));
    }

    /// Cortada pelo limite de saída (o backend diz): o motivo é esse, sem
    /// deduzir pelo JSON.
    #[tokio::test]
    async fn output_cut_by_the_limit_says_so() {
        let api = FakeApi::new(vec![
            Scripted::Invalid(r#"{"kind":"query","language":"sql","query":"SELECT"#, Some(StopReason::MaxTokens)),
            Scripted::Output(output("query", Some("SELECT 1"))),
        ]);

        let run = run(&api, &FakeDb::default(), &setup(true), &CancelFlag::default()).await;

        assert_eq!(run.finish.status, AssistantTurnStatus::Answered);
        let correction = &api.requests.lock()[1].messages[3].content;
        assert!(correction.contains(output::OUTPUT_LIMIT_REASON), "{correction}");
    }

    /// Um backend que não grava (direto: chave do usuário, modelo local): a
    /// tentativa vai ao journal antes do outcome dela.
    #[tokio::test]
    async fn unrecorded_attempts_go_to_the_journal_before_the_outcome() {
        let mut api = FakeApi::new(vec![
            Scripted::Invalid("not json", None),
            Scripted::Output(output("query", Some("SELECT count(*) FROM orders"))),
        ]);
        api.unrecorded = true;

        let run = run(&api, &FakeDb::default(), &setup(true), &CancelFlag::default()).await;

        assert_eq!(run.finish.status, AssistantTurnStatus::Answered);
        assert_eq!(*api.journal.lock(), ["record:attempt-0", "record:attempt-1", "outcome:attempt-1"]);
    }

    #[tokio::test]
    async fn provider_and_quota_errors_fail_the_turn() {
        let api = FakeApi::new(vec![Scripted::StreamError(AssistantCode::ProviderError)]);
        let run1 = run(&api, &FakeDb::default(), &setup(true), &CancelFlag::default()).await;
        assert_eq!(run1.finish.status, AssistantTurnStatus::Failed);
        assert_eq!(run1.finish.error_code.as_deref(), Some("ASSISTANT_PROVIDER_ERROR"));

        let api = FakeApi::new(vec![Scripted::Before(AssistantError::new(AssistantCode::QuotaExceeded, "q"))]);
        let run2 = run(&api, &FakeDb::default(), &setup(true), &CancelFlag::default()).await;
        assert_eq!(run2.finish.status, AssistantTurnStatus::Failed);
        assert_eq!(run2.finish.error_code.as_deref(), Some("ASSISTANT_QUOTA_EXCEEDED"));
    }

    #[tokio::test]
    async fn connection_failure_stops_without_correction() {
        let api = FakeApi::new(vec![Scripted::Output(output("query", Some("SELECT 1")))]);
        let db = FakeDb::default();
        db.preflight_errors.lock().push_back(Error::Connection("refused".into()));

        let run = run(&api, &db, &setup(true), &CancelFlag::default()).await;

        assert_eq!(run.finish.status, AssistantTurnStatus::Failed);
        assert_eq!(run.finish.error_code.as_deref(), Some("CONNECTION_ERROR"));
        assert_eq!(api.requests.lock().len(), 1);
        let preflight = &api.outcomes.lock()[0].1.preflight;
        assert!(preflight.ran && preflight.failure_class.is_none() && preflight.error_sanitized.is_none());
    }

    #[tokio::test]
    async fn rate_limit_waits_then_retries() {
        let mut limited = AssistantError::new(AssistantCode::RateLimited, "slow down");
        limited.retry_after = Some(Duration::from_millis(10));
        let api = FakeApi::new(vec![
            Scripted::Before(limited),
            Scripted::Output(output("query", Some("SELECT 1"))),
        ]);

        let run = run(&api, &FakeDb::default(), &setup(true), &CancelFlag::default()).await;

        assert_eq!(run.finish.status, AssistantTurnStatus::Answered);
        // Menos de um segundo aparece como 1 na tela
        assert!(run.events.iter().any(|e| matches!(e, AssistantTurnEvent::Waiting { seconds: 1 })));
    }

    #[tokio::test]
    async fn cancel_while_the_model_is_generating() {
        let api = Arc::new(FakeApi::new(vec![Scripted::Hang]));
        let db = Arc::new(FakeDb::default());
        let cancel = CancelFlag::default();
        let setup = setup(true);

        let running = spawn_turn(&api, &db, setup, &cancel, &ReplySlot::default());
        tokio::time::sleep(Duration::from_millis(20)).await;
        cancel.cancel();
        let finish = tokio::time::timeout(Duration::from_secs(2), running).await.unwrap().unwrap().unwrap();

        assert_eq!(finish.status, AssistantTurnStatus::Cancelled);
        assert_eq!(*api.finished.lock(), Some(AssistantTurnStatus::Cancelled));
    }

    #[tokio::test]
    async fn cancel_in_the_middle_of_the_execution() {
        let api = Arc::new(FakeApi::new(vec![Scripted::Output(output("query", Some("SELECT count(*) FROM big_table")))]));
        let db = Arc::new(FakeDb {
            hang: true,
            ..FakeDb::default()
        });
        let cancel = CancelFlag::default();
        let setup = setup(true);

        let running = spawn_turn(&api, &db, setup, &cancel, &ReplySlot::default());
        tokio::time::sleep(Duration::from_millis(20)).await;
        cancel.cancel();
        let finish = tokio::time::timeout(Duration::from_secs(3), running).await.unwrap().unwrap().unwrap();

        assert_eq!(finish.status, AssistantTurnStatus::Cancelled);
        // O banco recebeu o cancelamento pela execução marcada
        assert_eq!(*db.cancelled.lock(), ["client-1"]);
        assert_eq!(*api.finished.lock(), Some(AssistantTurnStatus::Cancelled));
    }

    #[test]
    fn history_takes_the_last_generated_answer_of_each_turn() {
        use crate::models::{AssistantAttempt, AssistantConversationSummary, AssistantModel, AssistantModelSelection, AssistantTurn};

        let attempt = |status, sql: Option<&str>| AssistantAttempt {
            id: "a".into(),
            index: 0,
            status,
            output: sql.map(|sql| {
                let value = match sql {
                    "?" => structure_request(&["sales.orders"]),
                    "!" => text_answer("Um índice acelera a busca."),
                    sql => output("query", Some(sql)),
                };
                output::parse(value, QueryLanguage::Sql).unwrap()
            }),
            usage: None,
            latency_ms: None,
            outcome: None,
            user_reply: None,
            created_at: String::new(),
        };
        let turn = |question: &str, attempts| AssistantTurn {
            summary: AssistantTurnSummary {
                id: "t".into(),
                index: 0,
                question: question.into(),
                language: crate::assistant::language::Language::Pt,
                status: AssistantTurnStatus::Answered,
                created_at: String::new(),
            },
            attempts,
            feedback: None,
        };
        let conversation = AssistantConversation {
            summary: AssistantConversationSummary {
                id: "c".into(),
                paradigm: "postgres".into(),
                model: AssistantModel { id: "m".into(), alias: "Modelo A".into(), context_window: 200_000 },
                selection_mode: AssistantModelSelection::Assigned,
                title: None,
                created_at: String::new(),
                updated_at: String::new(),
            },
            turns: vec![
                turn("primeira", vec![
                    attempt(AssistantAttemptStatus::Generated, Some("SELECT 1")),
                    attempt(AssistantAttemptStatus::Generated, Some("SELECT 2")),
                ]),
                turn("sem resposta", vec![attempt(AssistantAttemptStatus::ProviderError, None)]),
                turn("terceira", vec![attempt(AssistantAttemptStatus::Generated, Some("SELECT 3"))]),
                // O turno que parou depois de um pedido de estrutura fica com
                // a resposta de antes dele, ou fora
                turn("quarta", vec![
                    attempt(AssistantAttemptStatus::Generated, Some("SELECT 4")),
                    attempt(AssistantAttemptStatus::Generated, Some("?")),
                ]),
                turn("só pedido", vec![attempt(AssistantAttemptStatus::Generated, Some("?"))]),
                turn("o que é índice?", vec![
                    attempt(AssistantAttemptStatus::Generated, Some("?")),
                    attempt(AssistantAttemptStatus::Generated, Some("!")),
                ]),
            ],
        };

        let history = history_from(&conversation);
        let pairs: Vec<_> = history
            .iter()
            .map(|turn| {
                let answer = turn.output.query.as_deref().unwrap_or(turn.output.explanation.as_str());
                (turn.question.as_str(), answer)
            })
            .collect();
        assert_eq!(
            pairs,
            [
                ("primeira", "SELECT 2"),
                ("terceira", "SELECT 3"),
                ("quarta", "SELECT 4"),
                ("o que é índice?", "Um índice acelera a busca."),
            ]
        );
    }
}
