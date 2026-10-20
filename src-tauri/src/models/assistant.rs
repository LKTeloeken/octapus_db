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
