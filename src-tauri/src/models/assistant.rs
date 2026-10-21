//! Modelos do assistente de consulta que cruzam o IPC. Os que vêm da API
//! (contrato 1.4.0, `openapi.yaml`) têm os mesmos nomes de campo em camelCase,
//! então o mesmo tipo desserializa a resposta e vai para o front.

use serde::{Deserialize, Serialize};

/// O que a tela do assistente precisa saber antes de qualquer outra chamada.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantStatus {
    /// O build (ou o ambiente) tem a URL da API.
    pub configured: bool,
    /// A API respondeu ao health check com o banco dela no ar.
    pub online: bool,
    /// Há token salvo para esta API (não confere se ainda é válido: um 401
    /// numa chamada apaga o token e o status volta a `false`).
    pub logged_in: bool,
    /// Versão mínima do app que a API aceita, quando ela informa.
    pub min_app_version: Option<String>,
    /// Este app está abaixo de `minAppVersion`: pedir atualização.
    pub update_required: bool,
}

impl AssistantStatus {
    pub fn not_configured() -> Self {
        Self {
            configured: false,
            online: false,
            logged_in: false,
            min_app_version: None,
            update_required: false,
        }
    }
}

/// `ParticipantSummary` do contrato: o participante é um pseudônimo.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantParticipant {
    pub id: String,
    pub label: String,
}

/// Como o modelo de cada conversa é escolhido (`ModelSelection`): sorteado
/// pela API, livre (o participante escolhe) ou fixo.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AssistantModelSelection {
    Assigned,
    Free,
    Pinned,
}

/// Versões do termo: `acceptedVersion` nula ou diferente de `currentVersion`
/// → mostrar o termo (RN-11). `currentVersion` nula = nenhum termo publicado,
/// o assistente fica indisponível.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantConsentState {
    pub current_version: Option<String>,
    pub accepted_version: Option<String>,
}

/// Cota de tokens do participante (RN-23).
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantQuota {
    pub limit_tokens: i64,
    pub used_tokens: i64,
    /// Reservado por tentativas em andamento.
    pub reserved_tokens: i64,
}

/// `GET /v1/me`.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantMe {
    pub participant: AssistantParticipant,
    pub model_selection: AssistantModelSelection,
    pub consent: AssistantConsentState,
    pub quota: AssistantQuota,
}

/// Termo de consentimento vigente (`GET /v1/consents/current`).
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantConsentTerm {
    pub version: String,
    /// Texto em Markdown.
    pub body: String,
    /// RFC 3339.
    pub published_at: String,
}

/// Modelo que o participante em `free` pode escolher (`ModelSummary`). O id
/// real do provedor nunca sai da API: só o apelido.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantModel {
    pub id: String,
    pub alias: String,
    /// Tokens de contexto, para o orçamento da estrutura (RN-26).
    pub context_window: i64,
}

/// O plano de uma consulta do assistente, sem executá-la (preflight): o
/// `EXPLAIN (FORMAT JSON)` do Postgres ou o `EXPLAIN QUERY PLAN` do SQLite.
/// Fica no app — vai para a tela de revisão, nunca para a API.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadOnlyPlan {
    /// Linhas que o planejador espera devolver (Postgres)
    pub estimated_rows: Option<f64>,
    /// Custo total estimado, na unidade do planejador (Postgres)
    pub estimated_cost: Option<f64>,
    /// O plano como o banco descreve: JSON no Postgres, uma linha por passo
    /// no SQLite
    pub plan: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// Conversas, turnos e tentativas (contrato 1.4.0): o histórico mora só no
// servidor (C-15). Status desconhecidos de uma API mais nova viram `Unknown`
// em vez de quebrar a leitura.
// ─────────────────────────────────────────────────────────────────────────────

/// `Deserialize` de um enum do contrato que aceita valor novo: o nome que não
/// é de nenhuma variante vira `$fallback`. (O `#[serde(other)]` faria o mesmo,
/// mas o ts-rs não o entende.) Os nomes vêm do próprio `Serialize`.
macro_rules! lenient_deserialize {
    ($ty:ident { $($variant:ident),* $(,)? } else $fallback:ident) => {
        impl<'de> Deserialize<'de> for $ty {
            fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let name = String::deserialize(deserializer)?;
                Ok([$($ty::$variant),*]
                    .into_iter()
                    .find(|variant| {
                        serde_json::to_value(variant).ok().and_then(|value| value.as_str().map(|s| s == name))
                            == Some(true)
                    })
                    .unwrap_or($ty::$fallback))
            }
        }
    };
}
pub(crate) use lenient_deserialize;

/// `TurnStatus` do contrato.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AssistantTurnStatus {
    Open,
    /// Consulta gerada (executada ou entregue para revisão)
    Answered,
    /// Código entregue sem execução (RN-14)
    CodeDelivered,
    /// O assistente pediu esclarecimento (C-08)
    Clarification,
    /// Limite de tentativas atingido (RN-24)
    Exhausted,
    /// Falha que não é do limite (provedor, cota, tempo, banco fora)
    Failed,
    /// Interrompido pelo usuário (C-18)
    Cancelled,
    /// Valor de uma API mais nova
    Unknown,
}

lenient_deserialize!(AssistantTurnStatus {
    Open, Answered, CodeDelivered, Clarification, Exhausted, Failed, Cancelled
} else Unknown);

/// `AttemptStatus` do contrato.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AssistantAttemptStatus {
    Generating,
    Generated,
    InvalidOutput,
    ProviderError,
    Cancelled,
    LimitExceeded,
    Refused,
    /// Valor de uma API mais nova
    Unknown,
}

lenient_deserialize!(AssistantAttemptStatus {
    Generating, Generated, InvalidOutput, ProviderError, Cancelled, LimitExceeded, Refused
} else Unknown);

/// Consumo de uma tentativa.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantUsage {
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_write_tokens: i64,
    pub cost_usd: f64,
}

/// Onde a consulta parou no app.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssistantStage {
    /// O guard de leitura recusou antes de ir ao banco
    Guard,
    /// O `EXPLAIN` (sem executar)
    Preflight,
    Execution,
}

/// O guard no outcome. Para `code` e `clarification` ele não roda: vai
/// `allowed: false` com o motivo `kind_code`/`kind_clarification`, que a
/// planilha separa da recusa de verdade.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GuardResult {
    pub allowed: bool,
    /// Código técnico (`multiple_statements`…), nunca trecho da consulta
    pub reason: Option<String>,
}

/// Uma etapa do outcome. `ran: false` não leva resultado (a API recusa).
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StageResult {
    pub ran: bool,
    pub ok: Option<bool>,
    pub failure_class: Option<crate::assistant::failure::FailureClass>,
    /// Erro sanitizado (RN-10)
    pub error_sanitized: Option<String>,
    pub duration_ms: Option<i64>,
}

impl StageResult {
    pub fn skipped() -> Self {
        Self {
            ran: false,
            ok: None,
            failure_class: None,
            error_sanitized: None,
            duration_ms: None,
        }
    }
}

/// O que o app fez com a consulta gerada (`AttemptOutcome`).
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptOutcome {
    pub guard: GuardResult,
    pub preflight: StageResult,
    pub execution: StageResult,
    /// Linhas devolvidas: metadado, nunca o conteúdo
    pub row_count: Option<i64>,
    pub auto_execute: bool,
    pub user_edited: bool,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackRating {
    Up,
    Down,
}

/// Motivos rápidos do 👎.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackReason {
    WrongResult,
    WrongQuery,
    DidNotUnderstand,
    UnclearExplanation,
    Slow,
    Other,
    /// Valor de uma API mais nova (não volta para a API)
    Unknown,
}

lenient_deserialize!(FeedbackReason {
    WrongResult, WrongQuery, DidNotUnderstand, UnclearExplanation, Slow, Other
} else Unknown);

/// Teto do comentário da avaliação, em caracteres (o mesmo da API).
pub const FEEDBACK_COMMENT_MAX: usize = 2000;

/// A avaliação que a tela manda (`assistant_feedback_set`); também é o corpo
/// do `PUT /v1/turns/{turnId}/feedback`, depois do `normalized`.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantFeedbackInput {
    pub rating: FeedbackRating,
    /// Só no 👎 (no 👍 são descartados)
    pub reasons: Vec<FeedbackReason>,
    /// Texto livre, até 2000 caracteres; só espaços = sem comentário
    pub comment: Option<String>,
    /// A tentativa cuja resposta estava na tela
    pub attempt_id: Option<String>,
}

impl AssistantFeedbackInput {
    /// O que a API aceita: motivos só no 👎, sem repetir e sem os
    /// desconhecidos; comentário aparado, vazio vira `None`. Comentário acima
    /// do teto é recusado aqui, sem ir à API.
    pub fn normalized(self) -> std::result::Result<Self, String> {
        let mut reasons: Vec<FeedbackReason> = Vec::new();
        if self.rating == FeedbackRating::Down {
            for reason in self.reasons {
                if reason != FeedbackReason::Unknown && !reasons.contains(&reason) {
                    reasons.push(reason);
                }
            }
        }
        let comment = self
            .comment
            .map(|comment| comment.trim().to_string())
            .filter(|comment| !comment.is_empty());
        if comment
            .as_ref()
            .is_some_and(|comment| comment.chars().count() > FEEDBACK_COMMENT_MAX)
        {
            return Err(format!("comment longer than {FEEDBACK_COMMENT_MAX} characters"));
        }
        Ok(Self {
            rating: self.rating,
            reasons,
            comment,
            attempt_id: self.attempt_id,
        })
    }
}

/// A avaliação de um turno (B11).
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantFeedback {
    pub rating: FeedbackRating,
    pub reasons: Vec<FeedbackReason>,
    pub comment: Option<String>,
    pub attempt_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantAttempt {
    pub id: String,
    pub index: i64,
    pub status: AssistantAttemptStatus,
    pub output: Option<crate::assistant::output::ModelOutput>,
    pub usage: Option<AssistantUsage>,
    pub latency_ms: Option<i64>,
    pub outcome: Option<AttemptOutcome>,
    pub created_at: String,
}

/// `TurnSummary`: o que `POST /turns` devolve.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantTurnSummary {
    pub id: String,
    pub index: i64,
    pub question: String,
    pub language: crate::assistant::language::Language,
    pub status: AssistantTurnStatus,
    pub created_at: String,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantTurn {
    #[serde(flatten)]
    pub summary: AssistantTurnSummary,
    pub attempts: Vec<AssistantAttempt>,
    pub feedback: Option<AssistantFeedback>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantConversationSummary {
    pub id: String,
    /// `postgres`, `mongodb`, `redis` ou `sqlite`
    pub paradigm: String,
    /// O modelo, decidido ao abrir e fixo na conversa
    pub model: AssistantModel,
    pub selection_mode: AssistantModelSelection,
    pub title: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// A conversa inteira, com turnos e tentativas (C-15).
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantConversation {
    #[serde(flatten)]
    pub summary: AssistantConversationSummary,
    pub turns: Vec<AssistantTurn>,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantConversationPage {
    pub items: Vec<AssistantConversationSummary>,
    pub next_cursor: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// O turno no app (B6): eventos pelo `Channel` do `assistant_turn_ask` e o
// desfecho que o comando devolve
// ─────────────────────────────────────────────────────────────────────────────

/// Uma menção com `@` na pergunta (`@schema.tabela`, `@schema`, `@tabela`).
/// A relação citada entra obrigatoriamente no contexto da estrutura; a que não
/// existe volta na tela como aviso, com o nome como foi digitado.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantMention {
    /// `None` no catálogo sem schema (SQLite, Mongo)
    pub schema: Option<String>,
    /// `None` quando a menção é do schema inteiro
    pub table: Option<String>,
}

/// O andamento de um turno, para a tela acompanhar.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AssistantTurnEvent {
    /// O turno existe no servidor
    #[serde(rename_all = "camelCase")]
    TurnOpened { turn_id: String },
    /// A estrutura que vai ao modelo (RN-26: `truncated` = recorte).
    /// `missingMentions`: as citadas com `@` que não existem no banco;
    /// `shortenedMentions`: as citadas que entraram encurtadas (sem algumas
    /// colunas, ou o schema sem a lista inteira das relações).
    #[serde(rename_all = "camelCase")]
    Context {
        tables: u32,
        truncated: bool,
        missing_mentions: Vec<AssistantMention>,
        shortened_mentions: Vec<AssistantMention>,
    },
    #[serde(rename_all = "camelCase")]
    AttemptStarted { attempt_id: String, index: i64 },
    /// Texto parcial do modelo, só para mostrar progresso
    Delta { text: String },
    /// A resposta conferida da tentativa
    #[serde(rename_all = "camelCase")]
    Output { attempt_id: String, output: crate::assistant::output::ModelOutput },
    /// A consulta parou numa etapa. `message` é o erro como o banco deu (fica
    /// na máquina do usuário); para a API vai a versão sanitizada.
    #[serde(rename_all = "camelCase")]
    StageFailed {
        attempt_id: String,
        stage: AssistantStage,
        failure_class: Option<crate::assistant::failure::FailureClass>,
        message: String,
    },
    /// O plano da consulta (preflight), para a revisão
    #[serde(rename_all = "camelCase")]
    Plan { attempt_id: String, plan: ReadOnlyPlan },
    /// A primeira página do resultado (execução automática)
    #[serde(rename_all = "camelCase")]
    Result { attempt_id: String, result: crate::models::QueryResult },
    /// Rate limit: esperando antes de tentar de novo
    Waiting { seconds: u32 },
}

/// Como o turno terminou.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssistantTurnFinish {
    pub turn_id: Option<String>,
    pub status: AssistantTurnStatus,
    /// A última tentativa com resposta (e a resposta dela): no limite
    /// esgotado, é a consulta entregue com a explicação (RN-24)
    pub attempt_id: Option<String>,
    pub output: Option<crate::assistant::output::ModelOutput>,
    /// Por que parou, quando não foi uma resposta (cota, provedor, banco…)
    pub error_code: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_contract_values_do_not_break_reading() {
        let status: AssistantTurnStatus = serde_json::from_str("\"code_delivered\"").unwrap();
        assert_eq!(status, AssistantTurnStatus::CodeDelivered);
        let status: AssistantTurnStatus = serde_json::from_str("\"paused\"").unwrap();
        assert_eq!(status, AssistantTurnStatus::Unknown);
        let status: AssistantAttemptStatus = serde_json::from_str("\"limit_exceeded\"").unwrap();
        assert_eq!(status, AssistantAttemptStatus::LimitExceeded);
        let language: crate::assistant::language::Language = serde_json::from_str("\"es\"").unwrap();
        assert_eq!(language, crate::assistant::language::Language::Other);
        assert!(serde_json::from_str::<AssistantTurnStatus>("3").is_err());
        let reason: FeedbackReason = serde_json::from_str("\"too_long\"").unwrap();
        assert_eq!(reason, FeedbackReason::Unknown);
    }

    fn feedback(rating: FeedbackRating, reasons: Vec<FeedbackReason>, comment: Option<&str>) -> AssistantFeedbackInput {
        AssistantFeedbackInput {
            rating,
            reasons,
            comment: comment.map(str::to_owned),
            attempt_id: None,
        }
    }

    #[test]
    fn feedback_keeps_reasons_only_on_down() {
        use FeedbackReason::*;
        let up = feedback(FeedbackRating::Up, vec![Slow], None).normalized().unwrap();
        assert!(up.reasons.is_empty());

        let down = feedback(FeedbackRating::Down, vec![Slow, Unknown, WrongQuery, Slow], None)
            .normalized()
            .unwrap();
        assert_eq!(down.reasons, [Slow, WrongQuery]);
    }

    #[test]
    fn feedback_comment_is_trimmed_and_capped() {
        let blank = feedback(FeedbackRating::Up, vec![], Some("   \n ")).normalized().unwrap();
        assert_eq!(blank.comment, None);
        let trimmed = feedback(FeedbackRating::Up, vec![], Some("  ok ")).normalized().unwrap();
        assert_eq!(trimmed.comment.as_deref(), Some("ok"));

        // O teto conta caracteres, não bytes (como a API, em runes)
        let at_limit = "é".repeat(FEEDBACK_COMMENT_MAX);
        assert!(feedback(FeedbackRating::Down, vec![], Some(&at_limit)).normalized().is_ok());
        let over = format!("{at_limit}x");
        assert!(feedback(FeedbackRating::Down, vec![], Some(&over)).normalized().is_err());
    }
}
