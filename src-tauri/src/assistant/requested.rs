//! A estrutura que o modelo pede no meio do turno (`kind: structure_request`,
//! contrato 1.5.0): ele não recebeu o que precisava no contexto (o banco é
//! grande demais para caber) e lista os nomes que quer ver.
//!
//! Quem busca é o app, nunca uma consulta escolhida pelo modelo, e só onde o
//! contexto da pergunta já busca:
//!
//! - **SQL** (Postgres, SQLite): os nomes são conferidos no catálogo
//!   ([`schema_context::confirm`], o mesmo das menções `@`) e só os
//!   confirmados viram DDL, pelo mesmo [`schema_context::render`];
//! - **Mongo**: as coleções confirmadas, com o formato inferido de uma amostra
//!   ([`shape_context::render`]);
//! - **Redis**: os padrões das chaves que casam com o prefixo pedido, já
//!   generalizado ([`prefix_match`]) — nenhuma chave completa nem valor.
//!
//! O que não existe volta ao modelo como "não encontrado", com o nome como ele
//! escreveu. O texto respeita o orçamento que o turno passa; o que não couber
//! é recorte (RN-26) e a tela avisa.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::adapters::redisdb::patterns::prefix_match;
use crate::adapters::DatabaseAdapter;
use crate::catalog::{Candidate, FLAT_SCHEMA};
use crate::error::Result;
use crate::models::{AssistantMention, DatabaseType};
use crate::services::{CatalogEntry, CatalogService};

use super::key_context;
use super::output::MAX_STRUCTURE_REQUEST_NAMES;
use super::schema_context::{self, estimate_tokens, quote_ident, Render};
use super::shape_context::{self, ShapeRender};

/// Teto da busca por prefixos no Redis numa rodada: cada prefixo é um `SCAN`
/// (até 5 s); os que passam ficam como não encontrados.
const KEYS_LOOKUP_TIMEOUT: Duration = Duration::from_secs(10);

/// O que o app achou para um pedido de estrutura.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestedStructure {
    /// O texto de `<requested_structure>`: a estrutura achada e a linha dos
    /// não encontrados
    pub text: String,
    /// Nomes pedidos (sem repetir, até o teto do contrato)
    pub requested: usize,
    pub found: usize,
    /// Os não encontrados, como o modelo escreveu
    pub missing: Vec<String>,
    /// O achado não coube inteiro no orçamento (RN-26)
    pub truncated: bool,
    pub tokens_estimate: usize,
}

/// Os nomes como vieram, sem espaço em volta, vazio nem repetição, até o teto
/// do contrato (a API já confere; aqui falha fechado).
pub fn clean_names(names: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    names
        .iter()
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty() && seen.insert(name.clone()))
        .take(MAX_STRUCTURE_REQUEST_NAMES)
        .collect()
}

/// O que um nome pedido achou no catálogo.
enum Found {
    Relation(String, String),
    Schema(String),
}

/// SQL e Mongo, pelo catálogo. `scope`: os schemas da conversa (onde um nome
/// sem schema é procurado, como no `search_path`).
pub async fn from_catalog(
    catalogs: &CatalogService,
    entry: &Arc<CatalogEntry>,
    db_type: DatabaseType,
    scope: &[String],
    names: &[String],
    budget_tokens: usize,
) -> Result<RequestedStructure> {
    let names = clean_names(names);
    let flat = scope.iter().all(|schema| schema == FLAT_SCHEMA);

    let mut relations: Vec<(String, String)> = Vec::new();
    let mut listed: Vec<String> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    for name in &names {
        let mut hit = None;
        for mention in alternatives(name, flat) {
            let confirmed = schema_context::confirm(catalogs, entry, scope, std::slice::from_ref(&mention)).await?;
            if let Some((schema, table)) = confirmed.relations.into_iter().next() {
                hit = Some(Found::Relation(schema, table));
                break;
            }
            if let Some(schema) = confirmed.schemas.into_iter().next() {
                hit = Some(Found::Schema(schema));
                break;
            }
        }
        match hit {
            Some(Found::Relation(schema, table)) if !relations.contains(&(schema.clone(), table.clone())) => {
                relations.push((schema, table))
            }
            Some(Found::Schema(schema)) if !listed.contains(&schema) => listed.push(schema),
            Some(_) => {}
            None => missing.push(name.clone()),
        }
    }

    let found = names.len() - missing.len();
    let (structure, truncated) = if relations.is_empty() && listed.is_empty() {
        (None, false)
    } else if db_type == DatabaseType::Mongodb {
        let context = mongo_text(catalogs, entry, &relations, budget_tokens).await?;
        (Some(context.ddl), context.truncated)
    } else {
        let context = sql_text(catalogs, entry, db_type, &relations, &listed, budget_tokens).await?;
        (Some(context.ddl), context.truncated)
    };
    Ok(finish(structure, names.len(), found, missing, truncated))
}

/// As leituras possíveis de um nome, na ordem em que são tentadas.
///
/// Com schema (Postgres): `schema.tabela` e `schema.*` (o schema inteiro); o
/// nome sozinho é a tabela no escopo e, se não houver, um schema. Sem schema
/// (SQLite, Mongo): o nome inteiro (coleção pode ter ponto) e, se ele tem
/// ponto, o que vem depois do primeiro (`main.orders` no SQLite).
fn alternatives(name: &str, flat: bool) -> Vec<AssistantMention> {
    let table = |schema: Option<String>, table: String| AssistantMention {
        schema,
        table: Some(table),
    };
    if flat {
        let mut list = vec![table(None, unquote(name))];
        if let Some((_, rest)) = split_qualified(name) {
            if !rest.is_empty() && rest != "*" {
                list.push(table(None, rest));
            }
        }
        return list;
    }
    match split_qualified(name) {
        Some((schema, rest)) if rest == "*" || rest.is_empty() => vec![AssistantMention {
            schema: Some(schema),
            table: None,
        }],
        Some((schema, rest)) => vec![table(Some(schema), rest)],
        None => vec![
            table(None, unquote(name)),
            AssistantMention {
                schema: Some(unquote(name)),
                table: None,
            },
        ],
    }
}

/// `schema.tabela`, com ou sem aspas duplas (`"Vendas"."Pedidos"`). `None`
/// quando não há ponto fora das aspas.
fn split_qualified(name: &str) -> Option<(String, String)> {
    let mut in_quotes = false;
    for (index, c) in name.char_indices() {
        match c {
            '"' => in_quotes = !in_quotes,
            '.' if !in_quotes => return Some((unquote(&name[..index]), unquote(&name[index + 1..]))),
            _ => {}
        }
    }
    None
}

/// Tira as aspas duplas de um identificador (`"a""b"` → `a"b`).
fn unquote(name: &str) -> String {
    let name = name.trim();
    match name.strip_prefix('"').and_then(|rest| rest.strip_suffix('"')) {
        Some(inner) => inner.replace("\"\"", "\""),
        None => name.to_string(),
    }
}

/// O DDL das relações confirmadas e a lista dos nomes de cada schema pedido
/// inteiro, com a regra das menções: todas entram, encurtadas se preciso.
async fn sql_text(
    catalogs: &CatalogService,
    entry: &Arc<CatalogEntry>,
    db_type: DatabaseType,
    relations: &[(String, String)],
    listed: &[String],
    budget_tokens: usize,
) -> Result<schema_context::SchemaContext> {
    let candidates = {
        let catalog = entry.read();
        let mut candidates: Vec<Candidate> = relations
            .iter()
            .filter_map(|(schema, table)| {
                catalog.find_relation(schema, table).map(|(table, kind)| Candidate {
                    schema: schema.clone(),
                    table,
                    kind,
                    score: 0,
                })
            })
            .collect();
        for schema in listed {
            for candidate in catalog.context_candidates(&[schema.as_str()], "", &[], &[]) {
                if !candidates.iter().any(|known| known.key() == candidate.key()) {
                    candidates.push(candidate);
                }
            }
        }
        candidates
    };
    let mut schemas: Vec<String> = Vec::new();
    for schema in relations.iter().map(|(schema, _)| schema).chain(listed) {
        if !schemas.contains(schema) {
            schemas.push(schema.clone());
        }
    }

    let foreign_keys = catalogs.foreign_keys(entry, relations, relations).await?;
    let details = catalogs.relation_details(entry, relations).await?;
    Ok(schema_context::render(&Render {
        db_type,
        schemas: &schemas,
        total_schemas: None,
        candidates: &candidates,
        chosen: relations,
        cited: relations,
        listed,
        details: &details,
        foreign_keys: &foreign_keys,
        budget_tokens,
    }))
}

/// As coleções confirmadas, com o formato de uma amostra de cada.
async fn mongo_text(
    catalogs: &CatalogService,
    entry: &Arc<CatalogEntry>,
    relations: &[(String, String)],
    budget_tokens: usize,
) -> Result<schema_context::SchemaContext> {
    let names: Vec<String> = relations.iter().map(|(_, collection)| collection.clone()).collect();
    let candidates: Vec<Candidate> = {
        let catalog = entry.read();
        relations
            .iter()
            .filter_map(|(schema, collection)| {
                catalog.find_relation(schema, collection).map(|(table, kind)| Candidate {
                    schema: schema.clone(),
                    table,
                    kind,
                    score: 0,
                })
            })
            .collect()
    };
    let shapes = catalogs.infer_shape(entry, &names).await?;
    let context = shape_context::render(&ShapeRender {
        candidates: &candidates,
        chosen: &names,
        cited: &names,
        shapes: &shapes,
        budget_tokens,
    });
    Ok(context)
}

/// Redis: os padrões das chaves sob cada prefixo pedido. O prefixo passa pela
/// generalização antes de virar `MATCH` (o modelo não sonda chave nenhuma), e
/// prefixo sem chave nenhuma é não encontrado.
pub async fn from_keys(adapter: &dyn DatabaseAdapter, names: &[String], budget_tokens: usize) -> Result<RequestedStructure> {
    let names = clean_names(names);
    let budget_chars = budget_tokens * 4;
    let deadline = Instant::now() + KEYS_LOOKUP_TIMEOUT;

    let mut sections: Vec<String> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    let mut truncated = false;
    let mut used = 0;
    for name in &names {
        let Some(glob) = prefix_match(name) else {
            missing.push(name.clone());
            continue;
        };
        if Instant::now() >= deadline {
            missing.push(name.clone());
            continue;
        }
        let sample = match adapter.sample_key_patterns(Some(&glob)).await {
            Ok(sample) => sample,
            Err(error) => {
                log::info!("assistente: amostra de um prefixo pedido falhou ({})", error.code());
                missing.push(name.clone());
                continue;
            }
        };
        if sample.patterns.is_empty() {
            missing.push(name.clone());
            continue;
        }

        let partial = sample.sampled >= crate::adapters::redisdb::patterns::SAMPLE_KEYS || sample.omitted > 0;
        let mut section = format!(
            "-- Key patterns matching {} ({}{} key(s) sampled)",
            quote_ident(&glob),
            if partial { "first " } else { "" },
            sample.sampled
        );
        let mut patterns: Vec<_> = sample.patterns.iter().collect();
        patterns.sort_by(|a, b| a.pattern.cmp(&b.pattern));
        let mut left_out = sample.omitted;
        for pattern in patterns {
            let line = key_context::line(pattern);
            let cost = line.chars().count() + 1;
            if used + section.chars().count() + cost <= budget_chars {
                section.push('\n');
                section.push_str(&line);
            } else {
                left_out += 1;
            }
        }
        if left_out > 0 {
            section.push_str(&format!("\n-- … {left_out} more pattern(s) not shown"));
        }
        truncated |= partial || left_out > 0;
        used += section.chars().count() + 1;
        sections.push(section);
    }

    let found = names.len() - missing.len();
    let structure = (!sections.is_empty()).then(|| format!("-- Redis\n{}", sections.join("\n")));
    Ok(finish(structure, names.len(), found, missing, truncated))
}

/// Junta a estrutura achada e a linha dos não encontrados.
fn finish(
    structure: Option<String>,
    requested: usize,
    found: usize,
    missing: Vec<String>,
    truncated: bool,
) -> RequestedStructure {
    let mut text = structure.unwrap_or_default();
    if !missing.is_empty() {
        if !text.is_empty() {
            text.push('\n');
        }
        let names: Vec<String> = missing.iter().map(|name| quote_ident(name)).collect();
        text.push_str(&format!("-- Not found: {}", names.join(", ")));
    }
    RequestedStructure {
        tokens_estimate: estimate_tokens(&text),
        text,
        requested,
        found,
        missing,
        truncated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mention(schema: Option<&str>, table: Option<&str>) -> AssistantMention {
        AssistantMention {
            schema: schema.map(str::to_string),
            table: table.map(str::to_string),
        }
    }

    #[test]
    fn names_are_trimmed_deduplicated_and_capped() {
        let names: Vec<String> = [" orders ", "orders", "", "  "]
            .iter()
            .map(|name| name.to_string())
            .chain((0..30).map(|i| format!("t{i}")))
            .collect();
        let clean = clean_names(&names);
        assert_eq!(clean[0], "orders");
        assert_eq!(clean.len(), MAX_STRUCTURE_REQUEST_NAMES);
        assert!(!clean[1..].contains(&"orders".to_string()));
    }

    #[test]
    fn each_name_has_its_readings() {
        assert_eq!(alternatives("sales.orders", false), [mention(Some("sales"), Some("orders"))]);
        assert_eq!(
            alternatives(r#""Vendas"."Pedidos.2024""#, false),
            [mention(Some("Vendas"), Some("Pedidos.2024"))]
        );
        assert_eq!(alternatives("tenant_42.*", false), [mention(Some("tenant_42"), None)]);
        assert_eq!(
            alternatives("orders", false),
            [mention(None, Some("orders")), mention(Some("orders"), None)]
        );
        // Sem schema: o nome inteiro primeiro (coleção com ponto)
        assert_eq!(
            alternatives("main.orders", true),
            [mention(None, Some("main.orders")), mention(None, Some("orders"))]
        );
        assert_eq!(alternatives("orders", true), [mention(None, Some("orders"))]);
    }

    #[test]
    fn missing_names_go_back_as_written() {
        let structure = finish(Some("-- PostgreSQL".into()), 3, 1, vec!["ghost".into(), "a\"b".into()], false);
        assert_eq!(structure.text, "-- PostgreSQL\n-- Not found: \"ghost\", \"a\"\"b\"");
        let none = finish(None, 1, 0, vec!["ghost".into()], false);
        assert_eq!(none.text, "-- Not found: \"ghost\"");
        assert_eq!(none.tokens_estimate, estimate_tokens(&none.text));
    }
}
