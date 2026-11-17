//! O que a rodada grava de cada teste ([`UnitRecord`]) e o que a tela
//! acompanha enquanto ele roda ([`LiveUnit`]). São os arquivos da rodada, dado
//! da pesquisa: podem levar a pergunta e a consulta. O log do app, nunca.

use serde::{Deserialize, Serialize};

use super::compare::{Comparison, Table};
use super::config::AskUser;
use super::plan::Unit;
use crate::assistant::failure::FailureClass;
use crate::assistant::language::Language;
use crate::assistant::output::{ModelOutput, OutputKind};
use crate::models::{AssistantModel, AssistantStage, AssistantUsage, AttemptOutcome};

/// O estado final de um teste que nem chegou a abrir o turno (API fora, sem
/// cota, banco fora): não é resultado, e a retomada roda de novo.
pub const NOT_OPENED: &str = "not_opened";
/// O turno foi interrompido com a rodada: a retomada roda de novo.
pub const CANCELLED: &str = "cancelled";
/// O turno falhou no meio (provedor, conexão): `--redo-failed` roda de novo.
pub const FAILED: &str = "failed";

/// Uma tentativa do turno, ligada à tentativa na API pelo `attempt_id`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptRecord {
    pub attempt_id: String,
    pub index: i64,
    /// O tipo da resposta; `null` = a tentativa não deu uma resposta válida
    pub kind: Option<OutputKind>,
    /// A consulta gerada, quando a resposta é uma
    pub query: Option<String>,
    /// A resposta inteira, como o turno a leu
    pub output: Option<ModelOutput>,
    /// O texto do modelo quando a saída não seguiu o contrato
    pub raw: Option<String>,
    /// O código do erro da geração (`ASSISTANT_INVALID_OUTPUT`…)
    pub generation_error: Option<String>,
    /// Conta no limite de tentativas do turno (pedido de estrutura, pergunta
    /// ao usuário e erro transitório do provedor não contam)
    pub counted: bool,
    /// Em que etapa a consulta falhou (guard, preflight, execução)
    pub failed_stage: Option<AssistantStage>,
    pub failure_class: Option<FailureClass>,
    /// O erro como o banco deu (fica só nos arquivos da rodada)
    pub error: Option<String>,
    /// O que o app registrou na API para a tentativa
    pub outcome: Option<AttemptOutcome>,
    pub row_count: Option<i64>,
    /// O resultado bate com o gabarito; `null` = nada foi executado
    pub correct: Option<bool>,
    /// Igual à consulta de referência (só PostgreSQL)
    pub exact_match: Option<bool>,
    pub usage: Option<AssistantUsage>,
    /// Da ida do pedido à resposta completa do modelo
    pub generation_ms: i64,
}

/// Uma pergunta do assistente ao usuário e o que a política fez com ela.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AskedUser {
    pub attempt_id: String,
    pub question: String,
    pub choices: Vec<String>,
    pub policy: AskUser,
    /// A resposta enviada; `null` = ficou sem resposta
    pub reply: Option<String>,
}

/// Um teste da rodada, uma linha do `units.jsonl`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnitRecord {
    pub key: String,
    #[serde(flatten)]
    pub unit: Unit,
    pub difficulty: String,
    /// O banco da pergunta, quando há um por conjunto de dados
    #[serde(default)]
    pub dataset: Option<String>,
    pub unanswerable: bool,
    /// O texto perguntado
    pub question: String,
    /// O idioma que o detector do app viu, gravado no turno da API
    pub detected_language: Language,
    /// O modelo que a API fixou na conversa
    pub model_info: Option<AssistantModel>,
    pub prompt_version: Option<String>,
    pub conversation_id: Option<String>,
    pub turn_id: Option<String>,
    pub started_at: String,
    pub finished_at: String,
    /// O estado final do turno (`answered`, `clarification`, `exhausted`…) ou
    /// [`NOT_OPENED`]
    pub status: String,
    pub error_code: Option<String>,
    /// O tempo total do turno, da abertura ao encerramento
    pub turn_ms: i64,
    /// Relações do banco no contexto da pergunta, e se ele foi recortado
    pub context_tables: Option<u32>,
    pub context_truncated: Option<bool>,
    pub attempts: Vec<AttemptRecord>,
    /// Tentativas que contam no limite do turno
    pub counted_attempts: u32,
    pub asked_user: Vec<AskedUser>,
    /// Acertou sem nenhuma correção
    pub correct_first: bool,
    /// Acertou ao fim do turno
    pub correct_final: bool,
    /// A última consulta gerada é igual à de referência (só PostgreSQL, em
    /// pergunta com gabarito)
    pub exact_match: Option<bool>,
    /// O resultado obtido e a comparação com o gabarito
    pub result: Option<Table>,
    pub comparison: Option<Comparison>,
}

impl UnitRecord {
    /// O teste vale como feito? Os que não abriram e os interrompidos voltam
    /// na retomada.
    pub fn is_result(&self) -> bool {
        self.status != NOT_OPENED && self.status != CANCELLED
    }
}

/// O gabarito de uma pergunta num paradigma, já executado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GoldResult {
    pub question_id: String,
    pub paradigm: String,
    pub query: String,
    pub table: Table,
}

/// A etapa em que um teste está.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// Abrindo a conversa e montando o contexto
    Preparing,
    Generating,
    /// Guard e preflight
    Validating,
    Executing,
    Comparing,
    /// Esperando o provedor (limite de pedidos, erro transitório)
    Waiting,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveAttempt {
    pub attempt_id: String,
    pub index: i64,
    pub kind: Option<OutputKind>,
    pub query: Option<String>,
    pub failed_stage: Option<AssistantStage>,
    pub failure_class: Option<FailureClass>,
    pub error: Option<String>,
}

/// Um teste em andamento.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveUnit {
    pub key: String,
    #[serde(flatten)]
    pub unit: Unit,
    pub question: String,
    pub difficulty: String,
    pub stage: Stage,
    pub started_at: String,
    pub attempts: Vec<LiveAttempt>,
}

pub fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}
