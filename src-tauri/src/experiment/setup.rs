//! Os auxiliares que o `assistant_turn_ask` usa para montar o turno, copiados
//! como estão: na `main` eles são privados em `commands/assistant.rs`, e o
//! executor não altera o assistente.
//!
//! A `feat/core-cli` já tirou essas funções de lá para `assistant/setup.rs`,
//! públicas e com o mesmo corpo. Depois do merge este arquivo sai e fica um
//! `use crate::assistant::setup::*`.

use std::sync::Arc;

use crate::adapters::DatabaseAdapter;
use crate::assistant::key_context;
use crate::assistant::schema_context::{self, ContextRequest, SchemaContext};
use crate::catalog::FLAT_SCHEMA;
use crate::error::Result;
use crate::models::{AdapterCapabilities, DatabaseType, QueryLanguage};
use crate::services::{CatalogEntry, CatalogService};

/// Primeira página do resultado do assistente.
pub const FIRST_PAGE_ROWS: i64 = 200;

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
