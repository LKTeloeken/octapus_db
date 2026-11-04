//! Onde o turno fica registrado ([`TurnJournal`]): hoje a API do experimento
//! (`intermediary.rs`), que guarda o histórico e os indicadores (RN-29/31).
//!
//! O turno abre aqui, cada tentativa ganha o outcome aqui e o turno encerra
//! aqui. A tentativa em si é gravada por quem a gerou quando pode (a API
//! intermediária); a de um backend direto chega pelo
//! [`record_attempt`](TurnJournal::record_attempt).

use async_trait::async_trait;

use super::backend::Generation;
use super::language::Detected;
use super::messages::AttemptRequest;
use crate::error::Result;
use crate::models::{AssistantConversation, AssistantTurnStatus, AssistantTurnSummary, AttemptOutcome};

#[async_trait]
pub trait TurnJournal: Send + Sync {
    /// A conversa inteira: o paradigma, o modelo e os turnos anteriores (o
    /// histórico das mensagens, C-12).
    async fn conversation(&self, conversation_id: &str) -> Result<AssistantConversation>;

    /// Abre o turno. O mesmo `client_turn_id` devolve o turno já aberto.
    async fn open_turn(
        &self,
        conversation_id: &str,
        question: &str,
        language: Detected,
        client_turn_id: &str,
    ) -> Result<AssistantTurnSummary>;

    /// Grava uma tentativa que o backend não gravou
    /// ([`Generation::recorded`] `false`), antes do outcome dela.
    async fn record_attempt(&self, turn_id: &str, request: &AttemptRequest, generation: &Generation) -> Result<()>;

    /// O que o app fez com a resposta da tentativa (repetir sobrescreve).
    async fn report_outcome(&self, attempt_id: &str, outcome: &AttemptOutcome) -> Result<()>;

    async fn finish_turn(&self, turn_id: &str, status: AssistantTurnStatus) -> Result<()>;
}
