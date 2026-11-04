//! Quem gera as tentativas ([`ModelBackend`]) e o que volta de cada uma
//! ([`Generation`]).
//!
//! Hoje o único backend é a API intermediária (`intermediary.rs`), que gera e
//! grava a tentativa numa chamada só. Um backend direto — a chave do próprio
//! usuário, um modelo local — gera aqui no app e deixa a gravação para o
//! [`TurnJournal`](super::journal::TurnJournal): por isso a [`Generation`] diz
//! se já foi gravada.

use async_trait::async_trait;
use serde::Deserialize;

use super::cancel::CancelFlag;
use super::error::AssistantError;
use super::messages::AttemptRequest;
use crate::error::Result;
use crate::models::{AssistantModel, AssistantUsage};

/// O progresso da geração, conforme chega.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttemptProgress {
    Started { attempt_id: String, index: i64 },
    Delta(String),
}

/// Por que o modelo parou (o mesmo `stopReason` normalizado da API).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// Terminou normalmente
    End,
    /// Cortado pelo limite de saída
    MaxTokens,
    /// O modelo recusou (filtro do provedor)
    Refusal,
    /// Outro motivo, ou um valor que este app não conhece
    #[serde(other)]
    Other,
}

/// Uma tentativa gerada.
#[derive(Debug, Clone, PartialEq)]
pub struct Generation {
    pub attempt_id: String,
    pub index: i64,
    /// A resposta em JSON (o turno confere o contrato com `output::parse`) ou
    /// o erro dela: `invalid_output`, `model_refused`, `provider_error`…
    pub result: std::result::Result<serde_json::Value, AssistantError>,
    /// O texto do modelo como chegou: numa saída inválida, é o que volta ao
    /// modelo na correção
    pub raw: String,
    /// O consumo, quando o backend informa
    pub usage: Option<AssistantUsage>,
    /// Por que o modelo parou, quando o backend informa (a API, desde o
    /// contrato 1.7.0)
    pub stop: Option<StopReason>,
    /// O backend já gravou a tentativa (a API intermediária grava ao gerar);
    /// `false` = o turno manda ao journal antes do outcome
    pub recorded: bool,
}

/// O modelo da conversa, do ponto de vista do turno: o que ele precisa para
/// os orçamentos de contexto. Vem do cadastro da API hoje; de um modelo
/// local ou da chave do usuário depois.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelProfile {
    /// Só para o log
    pub id: String,
    /// Tokens de contexto (RN-26)
    pub context_window: i64,
}

impl From<&AssistantModel> for ModelProfile {
    fn from(model: &AssistantModel) -> Self {
        Self {
            id: model.id.clone(),
            context_window: model.context_window,
        }
    }
}

/// Quem gera as tentativas do turno.
#[async_trait]
pub trait ModelBackend: Send + Sync {
    /// Gera uma tentativa com as mensagens de `request`, mandando o progresso
    /// a `progress`. Cancelar interrompe a geração (`Error::Cancelled`).
    async fn generate(
        &self,
        turn_id: &str,
        request: &AttemptRequest,
        cancel: &CancelFlag,
        progress: &(dyn Fn(AttemptProgress) + Send + Sync),
    ) -> Result<Generation>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stop_reason_reads_the_api_values() {
        let read = |value: &str| serde_json::from_value::<StopReason>(serde_json::json!(value)).unwrap();
        assert_eq!(read("end"), StopReason::End);
        assert_eq!(read("max_tokens"), StopReason::MaxTokens);
        assert_eq!(read("refusal"), StopReason::Refusal);
        // Valor de uma API mais nova
        assert_eq!(read("tool_use"), StopReason::Other);
    }
}
