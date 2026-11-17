//! Um teste: o turno de verdade do assistente (`turn::run_turn`), com as
//! portas embrulhadas só para anotar o que passou por elas, e depois o
//! veredito contra o gabarito.
//!
//! Nada aqui muda o turno: os embrulhos repassam cada chamada como veio (o
//! mesmo jeito do `Recording` dos e2e), e a política de pergunta ao usuário
//! usa o que o turno já oferece: o tempo de espera da resposta e o
//! [`ReplySlot`].

use std::collections::HashMap;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use parking_lot::Mutex;

use super::compare::{self, Rules, Table};
use super::config::AskUser;
use super::record::{
    now, AskedUser, AttemptRecord, GoldResult, LiveAttempt, LiveUnit, Stage, UnitRecord, CANCELLED, NOT_OPENED,
};
use crate::assistant::backend::{AttemptProgress, Generation, ModelBackend};
use crate::assistant::journal::TurnJournal;
use crate::assistant::language::Detected;
use crate::assistant::messages::AttemptRequest;
use crate::assistant::output::{self, ModelOutput, OutputKind};
use crate::assistant::turn::{self, TurnPorts, TurnSetup};
use crate::assistant::{AssistantCode, CancelFlag, ReplySlot};
use crate::error::Result;
use crate::models::{
    AssistantConversation, AssistantTurnEvent, AssistantTurnStatus, AssistantTurnSummary,
    AssistantUsage, AttemptOutcome, QueryLanguage,
};

/// Quem acompanha um teste em andamento (a rodada, para o `live.json`).
pub type Observer<'a> = &'a (dyn Fn(&LiveUnit) + Send + Sync);

/// O gabarito de um teste e as regras para comparar com ele.
#[derive(Clone, Copy)]
pub struct Expected<'a> {
    pub gold: &'a GoldResult,
    pub rules: Rules,
}

/// Um teste pronto para rodar.
pub struct Job<'a> {
    /// O registro já com o que se sabe antes do turno
    pub record: UnitRecord,
    pub ports: TurnPorts<'a>,
    pub setup: TurnSetup,
    /// `None` = pergunta sem resposta possível
    pub expected: Option<Expected<'a>>,
    pub ask_user: AskUser,
    /// O texto da política `reply`
    pub reply: Option<&'a str>,
}

/// O que o backend devolveu numa tentativa.
struct Generated {
    attempt_id: String,
    index: i64,
    output: Option<ModelOutput>,
    raw: String,
    /// O código do erro e se o provedor o deu como transitório
    error: Option<(String, bool)>,
    usage: Option<AssistantUsage>,
    ms: i64,
}

/// O backend do turno, anotando cada tentativa gerada.
struct RecordingBackend<'a> {
    inner: &'a dyn ModelBackend,
    expected: QueryLanguage,
    generated: &'a Mutex<Vec<Generated>>,
}

#[async_trait]
impl ModelBackend for RecordingBackend<'_> {
    async fn generate(
        &self,
        turn_id: &str,
        request: &AttemptRequest,
        cancel: &CancelFlag,
        progress: &(dyn Fn(AttemptProgress) + Send + Sync),
    ) -> Result<Generation> {
        let started = Instant::now();
        let result = self.inner.generate(turn_id, request, cancel, progress).await;
        if let Ok(generation) = &result {
            self.generated.lock().push(Generated {
                attempt_id: generation.attempt_id.clone(),
                index: generation.index,
                // A mesma leitura que o turno faz da resposta
                output: generation
                    .result
                    .as_ref()
                    .ok()
                    .and_then(|value| output::parse(value.clone(), self.expected).ok()),
                raw: generation.raw.clone(),
                error: generation
                    .result
                    .as_ref()
                    .err()
                    .map(|error| (error.code.app_code().to_string(), is_transient(error.code, error.retryable))),
                usage: generation.usage.clone(),
                ms: started.elapsed().as_millis() as i64,
            });
        }
        result
    }
}

fn is_transient(code: AssistantCode, retryable: bool) -> bool {
    code == AssistantCode::ProviderError && retryable
}

/// O journal do turno, anotando o outcome que o app manda de cada tentativa.
struct RecordingJournal<'a> {
    inner: &'a dyn TurnJournal,
    outcomes: &'a Mutex<HashMap<String, AttemptOutcome>>,
}

#[async_trait]
impl TurnJournal for RecordingJournal<'_> {
    async fn conversation(&self, conversation_id: &str) -> Result<AssistantConversation> {
        self.inner.conversation(conversation_id).await
    }

    async fn open_turn(
        &self,
        conversation_id: &str,
        question: &str,
        language: Detected,
        client_turn_id: &str,
    ) -> Result<AssistantTurnSummary> {
        self.inner.open_turn(conversation_id, question, language, client_turn_id).await
    }

    async fn record_attempt(&self, turn_id: &str, request: &AttemptRequest, generation: &Generation) -> Result<()> {
        self.inner.record_attempt(turn_id, request, generation).await
    }

    async fn report_outcome(&self, attempt_id: &str, outcome: &AttemptOutcome) -> Result<()> {
        self.outcomes.lock().insert(attempt_id.to_string(), outcome.clone());
        self.inner.report_outcome(attempt_id, outcome).await
    }

    async fn finish_turn(&self, turn_id: &str, status: AssistantTurnStatus) -> Result<()> {
        self.inner.finish_turn(turn_id, status).await
    }
}

/// O que os eventos do turno contaram.
struct Trace {
    live: LiveUnit,
    results: HashMap<String, Table>,
    asked: Vec<AskedUser>,
    context: Option<(u32, bool)>,
}

impl Trace {
    fn attempt(&mut self, attempt_id: &str) -> Option<&mut LiveAttempt> {
        self.live.attempts.iter_mut().find(|attempt| attempt.attempt_id == attempt_id)
    }
}

/// Roda o teste e devolve o registro completo dele.
pub async fn run(job: Job<'_>, cancel: &CancelFlag, observe: Observer<'_>) -> UnitRecord {
    let Job {
        mut record,
        ports,
        mut setup,
        expected,
        ask_user,
        reply,
    } = job;

    // Política de pergunta ao usuário. `clarify`: ninguém responde, e o turno
    // encerra sozinho como esclarecimento. `reply`: a resposta fixa chega
    // pelo mesmo caminho da tela.
    setup.reply_timeout = match ask_user {
        AskUser::Clarify => Duration::ZERO,
        AskUser::Reply => turn::REPLY_TIMEOUT,
    };

    let generated: Mutex<Vec<Generated>> = Mutex::default();
    let outcomes: Mutex<HashMap<String, AttemptOutcome>> = Mutex::default();
    let backend = RecordingBackend {
        inner: ports.backend,
        expected: setup.expected_language,
        generated: &generated,
    };
    let journal = RecordingJournal {
        inner: ports.journal,
        outcomes: &outcomes,
    };
    let ports = TurnPorts {
        backend: &backend,
        journal: &journal,
        db: ports.db,
        structure: ports.structure,
    };

    let trace = Mutex::new(Trace {
        live: LiveUnit {
            key: record.key.clone(),
            unit: record.unit.clone(),
            question: record.question.clone(),
            difficulty: record.difficulty.clone(),
            stage: Stage::Generating,
            started_at: record.started_at.clone(),
            attempts: Vec::new(),
        },
        results: HashMap::new(),
        asked: Vec::new(),
        context: None,
    });
    let replies = ReplySlot::default();
    let auto_execute = setup.auto_execute;

    let emit = |event: AssistantTurnEvent| {
        let snapshot = {
            let mut trace = trace.lock();
            match event {
                AssistantTurnEvent::Delta { .. } | AssistantTurnEvent::UserReplied { .. } => return,
                AssistantTurnEvent::TurnOpened { .. } => trace.live.stage = Stage::Generating,
                AssistantTurnEvent::Context { tables, truncated, .. } => {
                    trace.context = Some((tables, truncated));
                    return;
                }
                AssistantTurnEvent::AttemptStarted { attempt_id, index } => {
                    trace.live.stage = Stage::Generating;
                    trace.live.attempts.push(LiveAttempt {
                        attempt_id,
                        index,
                        kind: None,
                        query: None,
                        failed_stage: None,
                        failure_class: None,
                        error: None,
                    });
                }
                AssistantTurnEvent::Waiting { .. } => trace.live.stage = Stage::Waiting,
                AssistantTurnEvent::StructureLookup { attempt_id, .. } => {
                    if let Some(attempt) = trace.attempt(&attempt_id) {
                        attempt.kind = Some(OutputKind::StructureRequest);
                    }
                }
                AssistantTurnEvent::Question {
                    attempt_id,
                    question,
                    choices,
                } => {
                    if let Some(attempt) = trace.attempt(&attempt_id) {
                        attempt.kind = Some(OutputKind::AskUser);
                    }
                    // A espera já está aberta quando a pergunta chega aqui
                    let sent = match (ask_user, reply) {
                        (AskUser::Reply, Some(text)) => replies.reply(text.to_string()).then(|| text.to_string()),
                        _ => None,
                    };
                    trace.asked.push(AskedUser {
                        attempt_id,
                        question,
                        choices,
                        policy: ask_user,
                        reply: sent,
                    });
                }
                AssistantTurnEvent::Output { attempt_id, output } => {
                    let is_query = output.kind == OutputKind::Query;
                    if let Some(attempt) = trace.attempt(&attempt_id) {
                        attempt.kind = Some(output.kind);
                        attempt.query = output.query;
                    }
                    if is_query {
                        trace.live.stage = Stage::Validating;
                    }
                }
                AssistantTurnEvent::StageFailed {
                    attempt_id,
                    stage,
                    failure_class,
                    message,
                } => {
                    if let Some(attempt) = trace.attempt(&attempt_id) {
                        attempt.failed_stage = Some(stage);
                        attempt.failure_class = failure_class;
                        attempt.error = Some(message);
                    }
                    trace.live.stage = Stage::Generating;
                }
                AssistantTurnEvent::Plan { .. } => {
                    if auto_execute {
                        trace.live.stage = Stage::Executing;
                    }
                }
                AssistantTurnEvent::Result { attempt_id, result } => {
                    trace.results.insert(attempt_id, Table::from(&result));
                    trace.live.stage = Stage::Comparing;
                }
            }
            trace.live.clone()
        };
        observe(&snapshot);
    };

    observe(&trace.lock().live.clone());
    let started = Instant::now();
    let finish = turn::run_turn(&ports, &setup, &emit, cancel, &replies).await;
    record.turn_ms = started.elapsed().as_millis() as i64;
    record.finished_at = now();

    let finish = match finish {
        Ok(finish) => finish,
        Err(error) => {
            // O turno nem abriu: não é resultado
            record.status = NOT_OPENED.into();
            record.error_code = Some(error.code().to_string());
            return record;
        }
    };

    let Trace {
        live,
        results,
        asked,
        context,
    } = trace.into_inner();
    let outcomes = outcomes.into_inner();
    let is_postgres = record.unit.paradigm == "postgres";

    let mut structure_requests = 0;
    let mut user_questions = 0;
    let mut provider_retries = 0;
    let mut last_result = None;
    for generation in generated.into_inner() {
        let Generated {
            attempt_id,
            index,
            output,
            raw,
            error,
            usage,
            ms,
        } = generation;
        let kind = output.as_ref().map(|output| output.kind);
        // As mesmas contas do turno: o que ele não desconta do limite
        let free = match (kind, &error) {
            (Some(OutputKind::StructureRequest), _) => bump(&mut structure_requests, turn::MAX_STRUCTURE_REQUESTS),
            (Some(OutputKind::AskUser), _) => bump(&mut user_questions, turn::MAX_USER_QUESTIONS),
            (_, Some((_, true))) => bump(&mut provider_retries, turn::PROVIDER_RETRIES),
            _ => false,
        };

        let seen = live.attempts.iter().find(|attempt| attempt.attempt_id == attempt_id);
        let outcome = outcomes.get(&attempt_id).cloned();
        let query = output.as_ref().and_then(|output| output.query.clone());
        let comparison = match (results.get(&attempt_id), expected) {
            (Some(table), Some(expected)) => Some(compare::compare(&expected.gold.table, table, &expected.rules)),
            _ => None,
        };
        let exact_match = match (&query, expected, kind) {
            (Some(query), Some(expected), Some(OutputKind::Query)) if is_postgres => {
                compare::exact_match(query, &expected.gold.query)
            }
            _ => None,
        };
        if let Some(table) = results.get(&attempt_id) {
            last_result = Some((table.clone(), comparison.clone()));
        }

        record.attempts.push(AttemptRecord {
            attempt_id,
            index,
            kind,
            query,
            raw: output.is_none().then_some(raw).filter(|raw| !raw.is_empty()),
            output,
            generation_error: error.map(|(code, _)| code),
            counted: !free,
            failed_stage: seen.and_then(|attempt| attempt.failed_stage),
            failure_class: seen.and_then(|attempt| attempt.failure_class),
            error: seen.and_then(|attempt| attempt.error.clone()),
            row_count: outcome.as_ref().and_then(|outcome| outcome.row_count),
            outcome,
            correct: comparison.as_ref().map(|comparison| comparison.equal),
            exact_match,
            usage,
            generation_ms: ms,
        });
    }

    record.status = status_name(finish.status);
    if finish.status == AssistantTurnStatus::Cancelled {
        record.status = CANCELLED.into();
    }
    record.error_code = finish.error_code;
    record.turn_id = finish.turn_id;
    record.context_tables = context.map(|(tables, _)| tables);
    record.context_truncated = context.map(|(_, truncated)| truncated);
    record.asked_user = asked;
    record.counted_attempts = record.attempts.iter().filter(|attempt| attempt.counted).count() as u32;

    record.correct_final = match expected {
        // Sem resposta possível: o acerto é o turno pedir esclarecimento
        None => finish.status == AssistantTurnStatus::Clarification,
        Some(_) => {
            finish.status == AssistantTurnStatus::Answered
                && record.attempts.last().is_some_and(|attempt| attempt.correct == Some(true))
        }
    };
    let corrections = record
        .attempts
        .iter()
        .rev()
        .skip(1)
        .filter(|attempt| attempt.counted)
        .count();
    record.correct_first = record.correct_final && corrections == 0;
    if let (true, Some(expected)) = (is_postgres, expected) {
        let last_query = record
            .attempts
            .iter()
            .rev()
            .find(|attempt| attempt.kind == Some(OutputKind::Query))
            .and_then(|attempt| attempt.query.as_deref());
        record.exact_match = Some(
            last_query
                .and_then(|query| compare::exact_match(query, &expected.gold.query))
                .unwrap_or(false),
        );
    }
    if let Some((table, comparison)) = last_result {
        record.result = Some(table);
        record.comparison = comparison;
    }
    record
}

/// Conta mais um do que o turno deixa passar sem contar como tentativa;
/// `false` quando o teto já foi atingido (a partir daí conta).
fn bump(used: &mut usize, limit: usize) -> bool {
    if *used < limit {
        *used += 1;
        true
    } else {
        false
    }
}

/// O estado final do turno com o nome do contrato (`code_delivered`…).
pub fn status_name(status: AssistantTurnStatus) -> String {
    serde_json::to_value(status)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_else(|| "unknown".into())
}

/// O registro de um teste antes de o turno rodar.
pub fn blank(base: Base<'_>) -> UnitRecord {
    UnitRecord {
        key: base.unit.key(),
        unit: base.unit.clone(),
        difficulty: base.difficulty.to_string(),
        dataset: base.dataset.map(str::to_string),
        unanswerable: base.unanswerable,
        question: base.question.to_string(),
        detected_language: base.detected.language,
        model_info: None,
        prompt_version: None,
        conversation_id: None,
        turn_id: None,
        started_at: now(),
        finished_at: String::new(),
        status: NOT_OPENED.into(),
        error_code: None,
        turn_ms: 0,
        context_tables: None,
        context_truncated: None,
        attempts: Vec::new(),
        counted_attempts: 0,
        asked_user: Vec::new(),
        correct_first: false,
        correct_final: false,
        exact_match: None,
        result: None,
        comparison: None,
    }
}

/// O que se sabe de um teste antes de ele rodar.
pub struct Base<'a> {
    pub unit: &'a super::plan::Unit,
    pub difficulty: &'a str,
    pub dataset: Option<&'a str>,
    pub unanswerable: bool,
    pub question: &'a str,
    pub detected: Detected,
}
