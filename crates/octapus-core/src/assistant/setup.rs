//! A montagem de um turno do assistente a partir do pedido da tela: os schemas
//! da conversa, as menções, a estrutura que vai ao modelo e o registro da
//! execução manual. Quem chama é o comando `assistant_turn_ask` do app (e,
//! depois, qualquer outra interface): aqui não há Tauri.

use std::sync::Arc;

use super::failure::{classify, sanitize};
use super::schema_context::{self, ContextRequest, SchemaContext};
use super::{key_context, ApiClient, AssistantService};
use crate::adapters::DatabaseAdapter;
use crate::catalog::FLAT_SCHEMA;
use crate::error::{Error, Result};
use crate::models::{
    AdapterCapabilities, AssistantMention, DatabaseType, QueryLanguage, QueryResult, StageResult,
};
use crate::services::{CatalogEntry, CatalogService};

// O que os e2e (`setup_e2e.rs`, `setup_privacy_e2e.rs`) alcançam por
// `use super::*`
#[cfg(test)]
#[allow(unused_imports)]
use {
    super::backend::ModelProfile,
    super::intermediary::Intermediary,
    super::journal::TurnJournal,
    super::read_only::AdapterDatabase,
    super::requested::RequestedLookup,
    super::turn::{TurnPorts, TurnSetup},
    super::{language, prompts, read_only, turn, AssistantCode, AssistantError},
    crate::adapters::ReadOnlyOp,
    crate::models::{
        AssistantConversation, AssistantFeedbackInput, AssistantTurnEvent, AssistantTurnFinish, QueryOptions,
    },
};

/// Primeira página do resultado do assistente.
pub const FIRST_PAGE_ROWS: i64 = 200;
/// Conversas por página na lista.
pub const CONVERSATIONS_PAGE: u32 = 30;

/// A opção de valores de exemplo, para o registro da tentativa: só onde ela
/// existe (capabilities; hoje o Mongo).
pub fn sample_values_option(db_type: DatabaseType, enabled: bool) -> Option<bool> {
    AdapterCapabilities::for_db_type(db_type)
        .is_some_and(|caps| caps.assistant_sample_values)
        .then_some(enabled)
}

/// O banco tem catálogo (capabilities)? Sem ele (Redis), a estrutura vem dos
/// padrões de chave de uma amostra do próprio adapter.
pub fn has_catalog(db_type: DatabaseType) -> bool {
    AdapterCapabilities::for_db_type(db_type).is_some_and(|caps| caps.has_catalog)
}

/// A estrutura que vai ao modelo: pelo catálogo (`entry`, no Postgres, SQLite
/// e Mongo) ou pelos padrões de chave (Redis, sem catálogo).
pub async fn structure_context(
    catalogs: &CatalogService,
    entry: Option<&Arc<CatalogEntry>>,
    db_type: DatabaseType,
    adapter: &dyn DatabaseAdapter,
    request: ContextRequest<'_>,
) -> Result<SchemaContext> {
    match entry {
        Some(entry) => schema_context::build(catalogs, entry, db_type, request).await,
        None => key_context::build(adapter, request).await,
    }
}

/// Uma execução manual de uma consulta do assistente, para o outcome.
pub struct ManualExecution<'a> {
    pub result: &'a Result<QueryResult>,
    pub query: &'a str,
    pub duration: Option<i64>,
    pub user_edited: bool,
}

/// Completa o outcome guardado da tentativa com a execução manual e manda de
/// novo (repetir sobrescreve). Sem o outcome em memória ou sem login, não faz
/// nada: é registro do experimento, não pode falhar a execução. O erro vai
/// sanitizado, como no turno (RN-10).
pub async fn report_manual_execution(
    assistant: &AssistantService,
    client: &ApiClient,
    token: &str,
    attempt_id: &str,
    execution: ManualExecution<'_>,
) {
    let ManualExecution {
        result,
        query,
        duration,
        user_edited,
    } = execution;
    let Some(mut outcome) = assistant.outcome(attempt_id) else {
        return;
    };
    match result {
        Ok(result) => {
            outcome.execution = StageResult {
                ran: true,
                ok: Some(true),
                failure_class: None,
                error_sanitized: None,
                duration_ms: duration,
            };
            outcome.row_count = Some(result.row_count as i64);
        }
        // Recusa do guard na revisão: a consulta editada não passou
        Err(Error::GuardRejected(_)) => return,
        Err(error) => {
            let class = classify(error);
            outcome.execution = StageResult {
                ran: true,
                ok: Some(false),
                failure_class: class,
                error_sanitized: class.and(sanitize(error, query)),
                duration_ms: duration,
            };
        }
    }
    outcome.user_edited = user_edited;

    match client.report_outcome(token, attempt_id, &outcome).await {
        Ok(()) => assistant.remember_outcome(attempt_id, &outcome),
        Err(error) => log::warn!(
            "assistente: outcome da execução manual de {attempt_id} não foi registrado ({})",
            error.code()
        ),
    }
}

/// Em que linguagem o assistente escreve para o banco (capabilities).
pub fn query_language(db_type: DatabaseType) -> QueryLanguage {
    AdapterCapabilities::for_db_type(db_type).map_or(QueryLanguage::Sql, |caps| caps.query_language)
}

/// Os schemas da conversa: no SQLite, no Mongo e no Redis, o schema sem nome;
/// no Postgres, os pedidos (sem repetir) ou o `public`.
pub fn conversation_schemas(db_type: DatabaseType, requested: Option<Vec<String>>) -> Vec<String> {
    if db_type != DatabaseType::Postgres {
        return vec![FLAT_SCHEMA.to_string()];
    }
    let mut schemas: Vec<String> = Vec::new();
    for schema in requested.unwrap_or_default() {
        let schema = schema.trim().to_string();
        if !schema.is_empty() && !schemas.contains(&schema) {
            schemas.push(schema);
        }
    }
    if schemas.is_empty() {
        schemas.push("public".to_string());
    }
    schemas
}

/// As menções como chegaram do front, sem nome vazio (vira `None`) nem as
/// que ficaram sem nome nenhum.
pub fn clean_mentions(mentions: Option<Vec<AssistantMention>>) -> Vec<AssistantMention> {
    let non_empty = |name: Option<String>| name.filter(|name| !name.is_empty());
    mentions
        .unwrap_or_default()
        .into_iter()
        .map(|mention| AssistantMention {
            schema: non_empty(mention.schema),
            table: non_empty(mention.table),
        })
        .filter(|mention| mention.schema.is_some() || mention.table.is_some())
        .collect()
}

/// Este app está abaixo da versão mínima que a API pede. Versão ilegível de
/// qualquer lado não bloqueia.
pub fn needs_update(min_app_version: Option<&str>, current: &str) -> bool {
    let (Some(min), Ok(current)) = (min_app_version, semver::Version::parse(current)) else {
        return false;
    };
    semver::Version::parse(min.trim_start_matches('v')).is_ok_and(|min| current < min)
}

#[cfg(test)]
mod tests {
    use super::needs_update;

    #[test]
    fn update_required_only_below_the_minimum() {
        assert!(needs_update(Some("1.2.0"), "1.1.0-beta.10"));
        assert!(needs_update(Some("1.1.0"), "1.1.0-beta.10"));
        assert!(!needs_update(Some("1.1.0-beta.10"), "1.1.0-beta.10"));
        assert!(!needs_update(Some("1.0.0"), "1.1.0-beta.10"));
        assert!(!needs_update(None, "1.1.0"));
        assert!(!needs_update(Some("garbage"), "1.1.0"));
    }
}

#[cfg(test)]
#[path = "setup_e2e.rs"]
mod e2e;

#[cfg(test)]
#[path = "setup_privacy_e2e.rs"]
mod privacy_e2e;
