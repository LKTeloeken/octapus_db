//! Assistente de consulta em linguagem natural (TCC): cliente da API
//! intermediária (octapus-assistant-api), que fala com os provedores de modelo
//! e registra o experimento. O loop do assistente roda no app; a API só gera
//! as tentativas e guarda o histórico.
//!
//! Nada do assistente vai para o disco do app além do token do participante,
//! cifrado pelo vault (`storage/repositories/assistant.rs`) — RN-01. No log,
//! só ids, contagens, durações e códigos de erro: nunca a pergunta, a
//! estrutura do banco ou a consulta gerada.

mod client;
mod config;
mod error;
pub mod guard;
pub mod read_only;

use std::sync::OnceLock;

pub use client::ApiClient;
pub use error::{AssistantCode, AssistantError};

use crate::error::{Error, Result};

/// A API configurada e o cliente HTTP, montado no primeiro uso.
pub struct AssistantService {
    base_url: Option<String>,
    client: OnceLock<ApiClient>,
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
        }
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
