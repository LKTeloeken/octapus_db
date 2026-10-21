//! O orquestrador de um turno (B6): a pergunta vira tentativas até uma
//! resposta, um esclarecimento, código para o usuário ou o limite.
//!
//! ```text
//! abrir o turno ─▶ tentativa (SSE) ─▶ resposta conferida
//!   clarification ─▶ encerra          code ─▶ entrega, nunca executa (RN-14)
//!   query ─▶ guard ─▶ preflight ─▶ execução (só com a automática ligada)
//!            └── falhou: outcome + erro sanitizado ─▶ nova tentativa (C-11)
//! limite da API (turn_limit_exceeded) ─▶ entrega a última consulta (RN-24)
//! ```
//!
//! Cada tentativa com resposta ganha um outcome na API. O loop depende de duas
//! portas — a API ([`TurnApi`]) e o banco ([`TurnDatabase`]) —, o que deixa os
//! testes rodarem com as duas simuladas. Log só com ids, contagens e códigos.

use std::time::{Duration, Instant};

use async_trait::async_trait;

use super::cancel::CancelFlag;
use super::client::{AttemptProgress, AttemptReply};
use super::error::AssistantCode;
use super::failure::{classify, sanitize, FailureClass};
use super::guard;
use super::language::Language;
use super::messages::{attempt_request, AttemptRequest, Correction, HistoryTurn};
use super::output::{self, ModelOutput, OutputKind};
use super::prompts::Prompt;
use super::schema_context::SchemaContext;
use crate::adapters::ReadOnlyOp;
use crate::error::{Error, Result};
use crate::models::{
    AssistantAttemptStatus, AssistantConversation, AssistantStage, AssistantTurnEvent,
    AssistantTurnFinish, AssistantTurnStatus, AssistantTurnSummary, AttemptOutcome, DatabaseType,
    GuardResult, QueryLanguage, QueryOptions, QueryResult, ReadOnlyPlan, StageResult,
};

/// Teto de tentativas do lado do app: a API corta antes (4 por turno, com as
/// correções), mas o loop não pode depender só dela.
const MAX_ATTEMPTS: usize = 6;
/// Rate limit antes do stream: quantas vezes esperar e tentar de novo.
const RATE_LIMIT_RETRIES: usize = 3;
/// Espera sem `Retry-After`.
const RATE_LIMIT_DEFAULT_WAIT: Duration = Duration::from_secs(5);
/// Depois de pedir o cancelamento ao banco, quanto esperar a consulta parar
/// antes de largá-la (o SQLite só para no próprio teto de tempo).
const CANCEL_GRACE: Duration = Duration::from_secs(2);

/// A API, do ponto de vista do turno.
#[async_trait]
pub trait TurnApi: Send + Sync {
    async fn create_turn(
        &self,
        conversation_id: &str,
        question: &str,
        language: Language,
        client_turn_id: &str,
    ) -> Result<AssistantTurnSummary>;

    async fn create_attempt(
        &self,
        turn_id: &str,
        request: &AttemptRequest,
        cancel: &CancelFlag,
        progress: &(dyn Fn(AttemptProgress) + Send + Sync),
    ) -> Result<AttemptReply>;

    async fn report_outcome(&self, attempt_id: &str, outcome: &AttemptOutcome) -> Result<()>;

    async fn finish_turn(&self, turn_id: &str, status: AssistantTurnStatus) -> Result<()>;
}

/// O banco do usuário, só leitura (as barreiras da B2 ficam no adapter).
#[async_trait]
pub trait TurnDatabase: Send + Sync {
    async fn preflight(&self, op: &ReadOnlyOp, page: QueryOptions) -> Result<ReadOnlyPlan>;
    async fn execute(&self, op: &ReadOnlyOp, page: QueryOptions) -> Result<QueryResult>;
    /// Interrompe a execução marcada com `query_id` (o `cancel_query` do app).
    async fn cancel(&self, query_id: &str);
}

/// Tudo que o turno precisa, montado antes (comando `assistant_turn_ask`).
pub struct TurnSetup {
    pub conversation_id: String,
    /// Idempotência na API e id da execução no banco (para cancelar)
    pub client_turn_id: String,
    pub question: String,
    pub language: Language,
    pub prompt: Prompt,
    pub db_type: DatabaseType,
    pub expected_language: QueryLanguage,
    pub schema: SchemaContext,
    pub history: Vec<HistoryTurn>,
    pub context_window: i64,
    pub auto_execute: bool,
    /// Primeira página da execução: `limit`, `schema` do `search_path` e o
    /// `query_id` (= `client_turn_id`)
    pub page: QueryOptions,
}

/// Os turnos anteriores para o histórico das mensagens (C-12): a pergunta e a
/// resposta da última tentativa gerada de cada um, na ordem da conversa.
pub fn history_from(conversation: &AssistantConversation) -> Vec<HistoryTurn> {
    conversation
        .turns
        .iter()
        .filter_map(|turn| {
            let output = turn
                .attempts
                .iter()
                .rev()
                .find(|attempt| attempt.status == AssistantAttemptStatus::Generated)?
                .output
                .clone()?;
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
    api: &dyn TurnApi,
    db: &dyn TurnDatabase,
    setup: &TurnSetup,
    emit: &(dyn Fn(AssistantTurnEvent) + Send + Sync),
    cancel: &CancelFlag,
) -> Result<AssistantTurnFinish> {
    let turn = api
        .create_turn(
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
    log::info!("assistente: turno {} aberto", turn.id);

    let mut corrections: Vec<Correction> = Vec::new();
    let mut last: Option<(String, ModelOutput)> = None;
    let mut retry_invalid = false;
    let mut attempts = 0;

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
            &corrections,
            setup.context_window,
        );
        if retry_invalid {
            request.context.previous_failure = Some(FailureClass::InvalidOutput);
        }

        let reply = match create_attempt(api, &turn.id, &request, cancel, emit).await {
            Ok(reply) => reply,
            Err(Error::Cancelled) => break (AssistantTurnStatus::Cancelled, None),
            Err(error) if is_api(&error, AssistantCode::TurnLimitExceeded) => {
                break (AssistantTurnStatus::Exhausted, None)
            }
            Err(error) => break (AssistantTurnStatus::Failed, Some(error.code().to_string())),
        };
        attempts += 1;
        let attempt_id = reply.attempt_id.clone();

        let value = match reply.result {
            Ok(value) => value,
            // A API já registrou a tentativa inválida: tenta de novo avisando
            Err(error) if error.code == AssistantCode::InvalidOutput => {
                log::info!("assistente: tentativa {attempt_id} com saída inválida");
                retry_invalid = true;
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
            Err(_) => {
                // Fora do contrato pelo lado do app (a linguagem não é a do banco)
                log::info!("assistente: tentativa {attempt_id} fora do contrato no app");
                report(api, &attempt_id, &not_run("invalid_output", setup.auto_execute)).await;
                retry_invalid = true;
                continue;
            }
        };
        retry_invalid = false;
        emit(AssistantTurnEvent::Output {
            attempt_id: attempt_id.clone(),
            output: output.clone(),
        });
        last = Some((attempt_id.clone(), output.clone()));

        match output.kind {
            OutputKind::Clarification => {
                report(api, &attempt_id, &not_run("kind_clarification", setup.auto_execute)).await;
                break (AssistantTurnStatus::Clarification, None);
            }
            OutputKind::Code => {
                // Código nunca roda (RN-14): só vai para a tela
                report(api, &attempt_id, &not_run("kind_code", setup.auto_execute)).await;
                break (AssistantTurnStatus::CodeDelivered, None);
            }
            OutputKind::Query => {
                let query = output.query.clone().unwrap_or_default();
                match run_query(api, db, setup, &attempt_id, &query, emit, cancel).await {
                    QueryEnd::Answered => break (AssistantTurnStatus::Answered, None),
                    QueryEnd::Cancelled => break (AssistantTurnStatus::Cancelled, None),
                    QueryEnd::Failed(code) => break (AssistantTurnStatus::Failed, Some(code)),
                    QueryEnd::Retry(mut correction) => {
                        correction.output = output;
                        corrections.push(correction);
                    }
                }
            }
        }
    };

    if let Err(error) = api.finish_turn(&turn.id, status).await {
        log::warn!("assistente: falha ao encerrar o turno {} ({})", turn.id, error.code());
    }
    log::info!(
        "assistente: turno {} encerrado ({status:?}, {attempts} tentativa(s))",
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

/// Pede a tentativa, esperando e repetindo quando a API pede calma (429).
async fn create_attempt(
    api: &dyn TurnApi,
    turn_id: &str,
    request: &AttemptRequest,
    cancel: &CancelFlag,
    emit: &(dyn Fn(AssistantTurnEvent) + Send + Sync),
) -> Result<AttemptReply> {
    let progress = |event: AttemptProgress| match event {
        AttemptProgress::Started { attempt_id, index } => {
            emit(AssistantTurnEvent::AttemptStarted { attempt_id, index })
        }
        AttemptProgress::Delta(text) => emit(AssistantTurnEvent::Delta { text }),
    };

    let mut waits = 0;
    loop {
        match api.create_attempt(turn_id, request, cancel, &progress).await {
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
    api: &dyn TurnApi,
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
            report(api, attempt_id, &outcome).await;
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
            return stage_error(api, attempt_id, &mut outcome, AssistantStage::Preflight, error, query, duration, emit).await;
        }
    }

    if !setup.auto_execute {
        // Revisão: a consulta e o plano ficam na tela; o usuário executa
        report(api, attempt_id, &outcome).await;
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
            report(api, attempt_id, &outcome).await;
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
            stage_error(api, attempt_id, &mut outcome, AssistantStage::Execution, error, query, duration, emit).await
        }
    }
}

/// Falha numa etapa do banco: da consulta (vai para a correção, com o erro
/// sanitizado) ou não (conexão, banco fora: o turno para).
#[allow(clippy::too_many_arguments)]
async fn stage_error(
    api: &dyn TurnApi,
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
    report(api, attempt_id, outcome).await;

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
async fn report(api: &dyn TurnApi, attempt_id: &str, outcome: &AttemptOutcome) {
    if let Err(error) = api.report_outcome(attempt_id, outcome).await {
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
    use crate::models::{AssistantTurnStatus, QueryColumnInfo};

    // ── API simulada ────────────────────────────────────────────────────────

    /// Cada pedido de tentativa consome a próxima resposta roteirizada.
    enum Scripted {
        Output(Value),
        StreamError(AssistantCode),
        Before(AssistantError),
        /// Fica pendurado até o cancelamento
        Hang,
    }

    #[derive(Default)]
    struct FakeApi {
        script: Mutex<VecDeque<Scripted>>,
        requests: Mutex<Vec<AttemptRequest>>,
        outcomes: Mutex<Vec<(String, AttemptOutcome)>>,
        finished: Mutex<Option<AssistantTurnStatus>>,
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
    impl TurnApi for FakeApi {
        async fn create_turn(&self, _: &str, question: &str, language: Language, _: &str) -> Result<AssistantTurnSummary> {
            Ok(AssistantTurnSummary {
                id: "turn-1".into(),
                index: 0,
                question: question.into(),
                language,
                status: AssistantTurnStatus::Open,
                created_at: "2026-10-20T00:00:00Z".into(),
            })
        }

        async fn create_attempt(
            &self,
            _: &str,
            request: &AttemptRequest,
            cancel: &CancelFlag,
            progress: &(dyn Fn(AttemptProgress) + Send + Sync),
        ) -> Result<AttemptReply> {
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
            match next {
                Scripted::Output(value) => {
                    started(progress);
                    Ok(AttemptReply { attempt_id: attempt_id.clone(), index, result: Ok(value) })
                }
                Scripted::StreamError(code) => {
                    started(progress);
                    Ok(AttemptReply {
                        attempt_id: attempt_id.clone(),
                        index,
                        result: Err(AssistantError::new(code, "x")),
                    })
                }
                Scripted::Before(error) => Err(error.into()),
                Scripted::Hang => {
                    started(progress);
                    cancel.cancelled().await;
                    Err(Error::Cancelled)
                }
            }
        }

        async fn report_outcome(&self, attempt_id: &str, outcome: &AttemptOutcome) -> Result<()> {
            self.outcomes.lock().push((attempt_id.into(), outcome.clone()));
            Ok(())
        }

        async fn finish_turn(&self, _: &str, status: AssistantTurnStatus) -> Result<()> {
            *self.finished.lock() = Some(status);
            Ok(())
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
        })
    }

    fn setup(auto_execute: bool) -> TurnSetup {
        TurnSetup {
            conversation_id: "conv-1".into(),
            client_turn_id: "client-1".into(),
            question: "quantos pedidos?".into(),
            language: Language::Pt,
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
            context_window: 200_000,
            auto_execute,
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
        let events: Arc<Mutex<Vec<AssistantTurnEvent>>> = Arc::default();
        let sink = Arc::clone(&events);
        let finish = run_turn(api, db, setup, &move |event| sink.lock().push(event), cancel)
            .await
            .unwrap();
        let events = events.lock().clone();
        Run { finish, events }
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

        let run = run(&api, &db, &setup, &CancelFlag::default()).await;

        assert_eq!(run.finish.status, AssistantTurnStatus::Answered);
        assert_eq!(*db.executed.lock(), [read]);
        assert_eq!(*db.languages.lock(), [QueryLanguage::Mongo]);
        let outcomes = api.outcomes.lock();
        assert_eq!(outcomes[0].1.guard.reason.as_deref(), Some("invalid_output"));
        assert_eq!(outcomes[1].1.guard.reason.as_deref(), Some("write_stage"));
        assert!(outcomes[2].1.execution.ok == Some(true));
        let requests = api.requests.lock();
        assert_eq!(requests[0].prompt_version, "mongo-v1");
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
        assert_eq!(requests[0].prompt_version, "redis-v1");
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

    #[tokio::test]
    async fn invalid_output_retries_with_the_failure_flagged() {
        let api = FakeApi::new(vec![
            Scripted::StreamError(AssistantCode::InvalidOutput),
            // Linguagem errada para o banco: inválida no app
            Scripted::Output(json!({
                "kind": "query", "language": "mongo", "query": "{}", "explanation": "",
                "assumptions": [], "clarificationQuestion": null,
            })),
            Scripted::Output(output("query", Some("SELECT 1"))),
        ]);
        let db = FakeDb::default();

        let run = run(&api, &db, &setup(true), &CancelFlag::default()).await;

        assert_eq!(run.finish.status, AssistantTurnStatus::Answered);
        let requests = api.requests.lock();
        assert_eq!(requests[1].context.previous_failure, Some(FailureClass::InvalidOutput));
        assert_eq!(requests[2].context.previous_failure, Some(FailureClass::InvalidOutput));
        assert_eq!(api.outcomes.lock()[0].1.guard.reason.as_deref(), Some("invalid_output"));
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

        let running = {
            let (api, db, cancel) = (Arc::clone(&api), Arc::clone(&db), cancel.clone());
            tokio::spawn(async move { run_turn(&*api, &*db, &setup, &|_| {}, &cancel).await })
        };
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

        let running = {
            let (api, db, cancel) = (Arc::clone(&api), Arc::clone(&db), cancel.clone());
            tokio::spawn(async move { run_turn(&*api, &*db, &setup, &|_| {}, &cancel).await })
        };
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
            output: sql.map(|sql| output::parse(output("query", Some(sql)), QueryLanguage::Sql).unwrap()),
            usage: None,
            latency_ms: None,
            outcome: None,
            created_at: String::new(),
        };
        let turn = |question: &str, attempts| AssistantTurn {
            summary: AssistantTurnSummary {
                id: "t".into(),
                index: 0,
                question: question.into(),
                language: Language::Pt,
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
            ],
        };

        let history = history_from(&conversation);
        let pairs: Vec<_> = history
            .iter()
            .map(|turn| (turn.question.as_str(), turn.output.query.as_deref().unwrap()))
            .collect();
        assert_eq!(pairs, [("primeira", "SELECT 2"), ("terceira", "SELECT 3")]);
    }
}
