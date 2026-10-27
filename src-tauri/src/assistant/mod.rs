//! Assistente de consulta em linguagem natural (TCC): cliente da API
//! intermediária (octapus-assistant-api), que fala com os provedores de modelo
//! e registra o experimento. O loop do assistente roda no app; a API só gera
//! as tentativas e guarda o histórico.
//!
//! Nada do assistente vai para o disco do app além do token do participante,
//! cifrado pelo vault (`storage/repositories/assistant.rs`) — RN-01. No log,
//! só ids, contagens, durações e códigos de erro: nunca a pergunta, a
//! estrutura do banco ou a consulta gerada.

mod cancel;
pub mod client;
mod config;
mod error;
pub mod failure;
pub mod guard;
pub mod key_context;
pub mod language;
pub mod literals;
pub mod messages;
pub mod output;
pub mod prompts;
pub mod read_only;
mod reply;
pub mod requested;
pub mod schema_context;
pub mod shape_context;
mod sse;
pub mod turn;

use std::collections::{HashMap, VecDeque};
use std::sync::OnceLock;

use parking_lot::Mutex;

pub use cancel::CancelFlag;
pub use client::ApiClient;
pub use error::{AssistantCode, AssistantError};
pub use reply::ReplySlot;

use crate::error::{Error, Result};
use crate::models::AttemptOutcome;

/// Outcomes guardados em memória (nunca em disco, RN-01), para a execução
/// manual da revisão completar o da tentativa.
const OUTCOMES_KEPT: usize = 256;

/// O que a tela alcança de um turno em andamento: a parada e a espera da
/// resposta a uma pergunta do modelo.
#[derive(Clone, Default)]
pub struct TurnHandle {
    pub cancel: CancelFlag,
    pub reply: ReplySlot,
}

/// A API configurada e o cliente HTTP, montado no primeiro uso; os turnos em
/// andamento (para cancelar e responder) e os últimos outcomes enviados.
pub struct AssistantService {
    base_url: Option<String>,
    client: OnceLock<ApiClient>,
    turns: Mutex<HashMap<String, TurnHandle>>,
    outcomes: Mutex<(HashMap<String, AttemptOutcome>, VecDeque<String>)>,
}

impl AssistantService {
    /// Lê a URL do ambiente ou do build (`config.rs`) uma vez, na partida.
    pub fn from_env() -> Self {
        let base_url = config::base_url();
        log::info!(
            "assistente: {}",
            if base_url.is_some() { "configurado" } else { "sem URL da API, desligado" }
        );
        Self {
            base_url,
            client: OnceLock::new(),
            turns: Mutex::default(),
            outcomes: Mutex::default(),
        }
    }

    /// Registra o turno (pelo `clientTurnId`) e devolve a parada e a espera
    /// da resposta dele.
    pub fn start_turn(&self, client_turn_id: &str) -> TurnHandle {
        let handle = TurnHandle::default();
        self.turns.lock().insert(client_turn_id.to_string(), handle.clone());
        handle
    }

    pub fn end_turn(&self, client_turn_id: &str) {
        self.turns.lock().remove(client_turn_id);
    }

    /// Cancela o turno em andamento; `false` se ele já terminou.
    pub fn cancel_turn(&self, client_turn_id: &str) -> bool {
        match self.turns.lock().get(client_turn_id) {
            Some(handle) => {
                handle.cancel.cancel();
                true
            }
            None => false,
        }
    }

    /// Entrega a resposta do usuário à pergunta do modelo; `false` se o turno
    /// já terminou ou não está esperando resposta.
    pub fn reply_turn(&self, client_turn_id: &str, text: String) -> bool {
        let handle = self.turns.lock().get(client_turn_id).cloned();
        handle.is_some_and(|handle| handle.reply.reply(text))
    }

    pub fn remember_outcome(&self, attempt_id: &str, outcome: &AttemptOutcome) {
        let mut guard = self.outcomes.lock();
        let (by_id, order) = &mut *guard;
        if by_id.insert(attempt_id.to_string(), outcome.clone()).is_none() {
            order.push_back(attempt_id.to_string());
        }
        while order.len() > OUTCOMES_KEPT {
            if let Some(oldest) = order.pop_front() {
                by_id.remove(&oldest);
            }
        }
    }

    pub fn outcome(&self, attempt_id: &str) -> Option<AttemptOutcome> {
        self.outcomes.lock().0.get(attempt_id).cloned()
    }

    /// A URL da API; `None` = assistente desligado neste build.
    pub fn base_url(&self) -> Option<&str> {
        self.base_url.as_deref()
    }

    /// A URL e o cliente, ou `ASSISTANT_NOT_CONFIGURED`.
    pub fn endpoint(&self) -> Result<(&str, &ApiClient)> {
        let base_url = self.base_url().ok_or(Error::AssistantNotConfigured)?;
        if let Some(client) = self.client.get() {
            return Ok((base_url, client));
        }
        let client = ApiClient::new(base_url)?;
        Ok((base_url, self.client.get_or_init(|| client)))
    }
}
