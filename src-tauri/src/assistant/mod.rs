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
pub mod messages;
pub mod output;
pub mod prompts;
pub mod read_only;
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

use crate::error::{Error, Result};
use crate::models::AttemptOutcome;

/// Outcomes guardados em memória (nunca em disco, RN-01), para a execução
/// manual da revisão completar o da tentativa.
const OUTCOMES_KEPT: usize = 256;

/// A API configurada e o cliente HTTP, montado no primeiro uso; os turnos em
/// andamento (para o cancelar) e os últimos outcomes enviados.
pub struct AssistantService {
    base_url: Option<String>,
    client: OnceLock<ApiClient>,
    turns: Mutex<HashMap<String, CancelFlag>>,
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

    /// Registra o turno (pelo `clientTurnId`) e devolve a parada dele.
    pub fn start_turn(&self, client_turn_id: &str) -> CancelFlag {
        let flag = CancelFlag::default();
        self.turns.lock().insert(client_turn_id.to_string(), flag.clone());
        flag
    }

    pub fn end_turn(&self, client_turn_id: &str) {
        self.turns.lock().remove(client_turn_id);
    }

    /// Cancela o turno em andamento; `false` se ele já terminou.
    pub fn cancel_turn(&self, client_turn_id: &str) -> bool {
        match self.turns.lock().get(client_turn_id) {
            Some(flag) => {
                flag.cancel();
                true
            }
            None => false,
        }
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
