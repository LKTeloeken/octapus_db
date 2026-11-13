//! A API intermediária (octapus-assistant-api) nas duas portas do turno: ela
//! gera as tentativas ([`ModelBackend`]), gravando cada uma ao gerar, e
//! guarda o turno ([`TurnJournal`]).

use async_trait::async_trait;

use super::backend::{AttemptProgress, Generation, ModelBackend};
use super::cancel::CancelFlag;
use super::client::ApiClient;
use super::journal::TurnJournal;
use super::language::Detected;
use super::messages::AttemptRequest;
use super::AssistantService;
use crate::error::{Error, Result};
use crate::models::{AssistantConversation, AssistantTurnStatus, AssistantTurnSummary, AttemptOutcome};

/// A API com o token do participante. O outcome enviado fica em memória
/// (`AssistantService`), para a execução manual completar depois.
pub struct Intermediary<'a> {
    pub client: &'a ApiClient,
    pub token: &'a str,
    pub assistant: &'a AssistantService,
}

#[async_trait]
impl ModelBackend for Intermediary<'_> {
    async fn generate(
        &self,
        turn_id: &str,
        request: &AttemptRequest,
        cancel: &CancelFlag,
        progress: &(dyn Fn(AttemptProgress) + Send + Sync),
    ) -> Result<Generation> {
        self.client
            .create_attempt(self.token, turn_id, request, cancel, progress)
            .await
    }
}

#[async_trait]
impl TurnJournal for Intermediary<'_> {
    async fn conversation(&self, conversation_id: &str) -> Result<AssistantConversation> {
        self.client.conversation(self.token, conversation_id).await
    }

    async fn open_turn(
        &self,
        conversation_id: &str,
        question: &str,
        language: Detected,
        client_turn_id: &str,
    ) -> Result<AssistantTurnSummary> {
        self.client
            .create_turn(self.token, conversation_id, question, language, client_turn_id)
            .await
    }

    /// A API só grava as tentativas que ela mesma gera (todas, hoje): a de
    /// um backend direto ainda não tem para onde ir.
    async fn record_attempt(&self, _: &str, _: &AttemptRequest, _: &Generation) -> Result<()> {
        Err(Error::UnsupportedType(
            "the assistant API only records the attempts it generates".into(),
        ))
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
