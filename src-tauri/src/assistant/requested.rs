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
//! Nos bancos com catálogo, um nome pode vir com prefixo (prompts v3): o
//! modelo pede objetos que não são relações, pelas mesmas funções do catálogo
//! que a sidebar usa ([`CatalogService::objects`] e [`CatalogService::definition`]):
//!
//! - `indexes:tabela` (ou `indexes:schema.*`): os índices, com método,
//!   colunas e papel;
//! - `functions:schema`: funções e procedures (Postgres), sem as de extensão;
//! - `definition:nome`: a definição de uma view, função ou índice, com os
//!   literais trocados por `?` ([`literals`]) — a definição pode trazer dado;
//! - `columns:schema.*termo*` (SQL): as colunas cujo nome casa, com a tabela e
//!   o tipo, pelo catálogo do banco ([`CatalogService::search_columns`]) — é
//!   o "onde está o CPF?". `columns:schema.tabela` é a própria tabela, como o
//!   nome sem prefixo.
//!
//! O que não existe volta ao modelo como "não encontrado", com o nome como ele
//! escreveu. O texto respeita o orçamento que o turno passa; o que não couber
//! é recorte (RN-26) e a tela avisa.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::adapters::redisdb::patterns::prefix_match;
use crate::adapters::DatabaseAdapter;
use crate::catalog::{
    Candidate, CatalogObject, ColumnMatch, FunctionKind, FunctionObject, IndexObject, ObjectKind, ObjectRef, RelKind,
    FLAT_SCHEMA,
};
use crate::error::{Error, Result};
use crate::models::{AssistantMention, DatabaseType};
use crate::services::{CatalogEntry, CatalogService};

use super::key_context;
use super::literals::{mask_json, mask_sql};
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

/// Teto de índices e funções por pedido (a lista é cortada pelo orçamento
/// antes disso, num schema grande)
const OBJECTS_LIMIT: usize = 500;

/// Quantas sobrecargas de uma função `definition:nome` traz, sem assinatura.
const MAX_OVERLOADS: usize = 5;

/// Teto de colunas por busca `columns:` (passou disso, o modelo estreita o
/// termo).
const COLUMNS_LIMIT: usize = 100;

/// O que se pede de um nome com prefixo (`indexes:`, `functions:`,
/// `definition:`), além da estrutura.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ObjectRequest {
    Indexes(String),
    Functions(String),
    Definition(String),
    Columns(String),
}

/// O prefixo de um nome pedido, sem diferenciar maiúsculas. `None` = nome
/// comum (uma relação, um schema).
fn object_request(name: &str) -> Option<ObjectRequest> {
    let (prefix, rest) = name.split_once(':')?;
    let rest = rest.trim().to_string();
    match prefix.trim().to_ascii_lowercase().as_str() {
        "indexes" => Some(ObjectRequest::Indexes(rest)),
        "functions" => Some(ObjectRequest::Functions(rest)),
        "definition" => Some(ObjectRequest::Definition(rest)),
        "columns" => Some(ObjectRequest::Columns(rest)),
        _ => None,
    }
}

/// Lugar do aviso `-- … n more line(s) not shown`, em caracteres.
const NOTICE_RESERVE: usize = 40;

/// Um bloco do texto: o cabeçalho e as linhas, cortadas pelo orçamento.
struct Section {
    header: String,
    lines: Vec<String>,
}

/// O que um nome pedido achou no catálogo.
enum Found {
    Relation(String, String),
    Schema(String),
}

/// Onde e como um pedido é buscado no catálogo.
pub struct Lookup<'a> {
    pub db_type: DatabaseType,
    /// Os schemas da conversa, onde um nome sem schema é procurado (como no
    /// `search_path`)
    pub scope: &'a [String],
    /// Valores de exemplo dos campos categóricos (Mongo), como no contexto
    pub sample_values: bool,
}

impl<'a> Lookup<'a> {
    #[cfg(test)]
    pub fn new(db_type: DatabaseType, scope: &'a [String]) -> Self {
        Self {
            db_type,
            scope,
            sample_values: false,
        }
    }
}

/// SQL e Mongo, pelo catálogo.
pub async fn from_catalog(
    catalogs: &CatalogService,
    entry: &Arc<CatalogEntry>,
    lookup: &Lookup<'_>,
    names: &[String],
    budget_tokens: usize,
) -> Result<RequestedStructure> {
    let Lookup { db_type, scope, .. } = *lookup;
    let names = clean_names(names);
    let flat = scope.iter().all(|schema| schema == FLAT_SCHEMA);

    let mut relations: Vec<(String, String)> = Vec::new();
    let mut listed: Vec<String> = Vec::new();
    let mut missing: Vec<String> = Vec::new();
    let mut sections: Vec<Section> = Vec::new();
    let mut plain = 0;
    for name in &names {
        let request = object_request(name);
        // `columns:tabela` é a tabela: vai com a estrutura, como o nome comum
        let relation = match &request {
            Some(ObjectRequest::Columns(target)) if !target.contains(['*', '?']) => {
                match resolve(catalogs, entry, scope, target, flat).await? {
                    Some(Found::Relation(schema, table)) => Some((schema, table)),
                    _ => None,
                }
            }
            _ => None,
        };
        if let Some(relation) = relation {
            plain += 1;
            if !relations.contains(&relation) {
                relations.push(relation);
            }
            continue;
        }
        if let Some(request) = request {
            let found = objects_of(catalogs, entry, db_type, scope, flat, &request).await?;
            if found.is_empty() {
                missing.push(name.clone());
            }
            sections.extend(found);
            continue;
        }
        plain += 1;
        match resolve(catalogs, entry, scope, name, flat).await? {
            Some(Found::Relation(schema, table)) if !relations.contains(&(schema.clone(), table.clone())) => {
                relations.push((schema, table))
            }
            Some(Found::Schema(schema)) if !listed.contains(&schema) => listed.push(schema),
            Some(_) => {}
            None => missing.push(name.clone()),
        }
    }

    let found = names.len() - missing.len();
    // A estrutura das relações fica com a parte do orçamento dos nomes comuns;
    // os objetos, com o resto (e o que sobrar dela)
    let structure_budget = if sections.is_empty() {
        budget_tokens
    } else {
        budget_tokens * plain / names.len().max(1)
    };
    let (structure, mut truncated) = if relations.is_empty() && listed.is_empty() {
        (None, false)
    } else if db_type == DatabaseType::Mongodb {
        let context = mongo_text(catalogs, entry, &relations, lookup.sample_values, structure_budget).await?;
        (Some(context.ddl), context.truncated)
    } else {
        let context = sql_text(catalogs, entry, db_type, &relations, &listed, structure_budget).await?;
        (Some(context.ddl), context.truncated)
    };
    let used = structure.as_deref().map_or(0, |text| text.chars().count() + 1);
    let (objects, cut) = render_sections(&sections, (budget_tokens * 4).saturating_sub(used));
    truncated |= cut;
    let structure = match (structure, objects) {
        (Some(structure), Some(objects)) => Some(format!("{structure}\n{objects}")),
        (structure, objects) => structure.or(objects),
    };
    Ok(finish(structure, names.len(), found, missing, truncated))
}

/// Um nome comum no catálogo: a relação (no escopo, se veio sem schema) ou o
/// schema inteiro.
async fn resolve(
    catalogs: &CatalogService,
    entry: &Arc<CatalogEntry>,
    scope: &[String],
    name: &str,
    flat: bool,
) -> Result<Option<Found>> {
    for mention in alternatives(name, flat) {
        let confirmed = schema_context::confirm(catalogs, entry, scope, std::slice::from_ref(&mention)).await?;
        if let Some((schema, table)) = confirmed.relations.into_iter().next() {
            return Ok(Some(Found::Relation(schema, table)));
        }
        if let Some(schema) = confirmed.schemas.into_iter().next() {
            return Ok(Some(Found::Schema(schema)));
        }
    }
    Ok(None)
}

/// O schema pedido sozinho (`app` ou `app.*`), como está no catálogo.
async fn resolve_schema(
    catalogs: &CatalogService,
    entry: &Arc<CatalogEntry>,
    scope: &[String],
    name: &str,
) -> Result<Option<String>> {
    let name = name.strip_suffix(".*").unwrap_or(name);
    if name.is_empty() {
        return Ok(None);
    }
    let mention = AssistantMention {
        schema: Some(unquote(name)),
        table: None,
    };
    let confirmed = schema_context::confirm(catalogs, entry, scope, &[mention]).await?;
    Ok(confirmed.schemas.into_iter().next())
}

/// Recurso que o banco não tem (funções no SQLite, índices do Mongo sem a
/// coleção): para o modelo, é como não existir.
fn unsupported_as_empty<T: Default>(result: Result<T>) -> Result<T> {
    match result {
        Err(Error::UnsupportedType(_)) => Ok(T::default()),
        other => other,
    }
}

async fn list_indexes(
    catalogs: &CatalogService,
    entry: &Arc<CatalogEntry>,
    schema: &str,
    table: Option<&str>,
) -> Result<Vec<IndexObject>> {
    let page = catalogs
        .objects(entry, ObjectKind::Index, schema, table, None, 0, OBJECTS_LIMIT)
        .await
        .map(|page| page.items);
    Ok(unsupported_as_empty(page)?
        .into_iter()
        .filter_map(|object| match object {
            CatalogObject::Index(index) => Some(index),
            _ => None,
        })
        .collect())
}

async fn list_functions(
    catalogs: &CatalogService,
    entry: &Arc<CatalogEntry>,
    schema: &str,
) -> Result<Vec<FunctionObject>> {
    let page = catalogs
        .objects(entry, ObjectKind::Function, schema, None, None, 0, OBJECTS_LIMIT)
        .await
        .map(|page| page.items);
    Ok(unsupported_as_empty(page)?
        .into_iter()
        .filter_map(|object| match object {
            CatalogObject::Function(function) => Some(function),
            _ => None,
        })
        .collect())
}

/// O que um pedido com prefixo achou, em blocos de texto. Vazio = não
/// encontrado.
async fn objects_of(
    catalogs: &CatalogService,
    entry: &Arc<CatalogEntry>,
    db_type: DatabaseType,
    scope: &[String],
    flat: bool,
    request: &ObjectRequest,
) -> Result<Vec<Section>> {
    match request {
        ObjectRequest::Indexes(target) => {
            if target.is_empty() {
                return Ok(Vec::new());
            }
            // Sem schema, `*` é o banco inteiro (menos no Mongo, que lista
            // os índices só por coleção)
            let found = if flat && target == "*" && db_type != DatabaseType::Mongodb {
                Some(Found::Schema(FLAT_SCHEMA.to_string()))
            } else {
                resolve(catalogs, entry, scope, target, flat).await?
            };
            Ok(match found {
                Some(Found::Relation(schema, table)) => {
                    let kind = entry.read().find_relation(&schema, &table).map(|(_, kind)| kind);
                    // View não tem índice (e o `listIndexes` do Mongo recusa)
                    let indexes = match kind {
                        Some(RelKind::View) => Vec::new(),
                        _ => list_indexes(catalogs, entry, &schema, Some(&table)).await?,
                    };
                    vec![indexes_section(&schema, Some(&table), &indexes)]
                }
                Some(Found::Schema(schema)) if db_type != DatabaseType::Mongodb => {
                    let indexes = list_indexes(catalogs, entry, &schema, None).await?;
                    vec![indexes_section(&schema, None, &indexes)]
                }
                _ => Vec::new(),
            })
        }
        ObjectRequest::Functions(target) => {
            if flat {
                return Ok(Vec::new());
            }
            let Some(schema) = resolve_schema(catalogs, entry, scope, target).await? else {
                return Ok(Vec::new());
            };
            let functions = list_functions(catalogs, entry, &schema).await?;
            Ok(vec![functions_section(&schema, &functions)])
        }
        ObjectRequest::Definition(target) => definitions_of(catalogs, entry, db_type, scope, flat, target).await,
        ObjectRequest::Columns(target) => columns_of(catalogs, entry, db_type, scope, flat, target).await,
    }
}

/// `columns:schema.*termo*` (ou `columns:*termo*` nos schemas da conversa e
/// no SQLite): as colunas cujo nome casa com o padrão, com `*` e `?` como no
/// glob; sem curinga, o nome exato. O Mongo não tem colunas para buscar (o
/// formato vem da amostra de cada coleção).
async fn columns_of(
    catalogs: &CatalogService,
    entry: &Arc<CatalogEntry>,
    db_type: DatabaseType,
    scope: &[String],
    flat: bool,
    target: &str,
) -> Result<Vec<Section>> {
    if db_type == DatabaseType::Mongodb || target.is_empty() {
        return Ok(Vec::new());
    }
    let (schemas, pattern) = match split_qualified(target) {
        Some((schema, pattern)) if !flat => match resolve_schema(catalogs, entry, scope, &schema).await? {
            Some(schema) => (vec![schema], pattern),
            None => return Ok(Vec::new()),
        },
        // `main.*cpf*` no SQLite: o que vem depois do ponto
        Some((_, pattern)) if flat => (vec![FLAT_SCHEMA.to_string()], pattern),
        _ if flat => (vec![FLAT_SCHEMA.to_string()], unquote(target)),
        _ => (scope.to_vec(), unquote(target)),
    };
    if pattern.is_empty() || pattern.chars().all(|c| c == '*' || c == '?') {
        return Ok(Vec::new());
    }

    let like = like_pattern(&pattern);
    let mut sections = Vec::new();
    for schema in &schemas {
        let found = catalogs.search_columns(entry, schema, &like, COLUMNS_LIMIT + 1).await;
        let mut found = unsupported_as_empty(found)?;
        let more = found.len() > COLUMNS_LIMIT;
        found.truncate(COLUMNS_LIMIT);
        sections.push(columns_section(schema, &pattern, &found, more));
    }
    Ok(sections)
}

/// O glob do modelo (`*cpf*`, `?d`) como `LIKE` com `\` de escape: os `%`,
/// `_` e `\` do nome valem como eles mesmos.
fn like_pattern(glob: &str) -> String {
    let mut like = String::with_capacity(glob.len() + 2);
    for c in glob.chars() {
        match c {
            '*' => like.push('%'),
            '?' => like.push('_'),
            '%' | '_' | '\\' => {
                like.push('\\');
                like.push(c);
            }
            c => like.push(c),
        }
    }
    like
}

fn columns_section(schema: &str, pattern: &str, found: &[ColumnMatch], more: bool) -> Section {
    let place = if schema == FLAT_SCHEMA {
        String::new()
    } else {
        format!(" in {}", quote_ident(schema))
    };
    let header = if found.is_empty() {
        format!("-- No columns matching {}{place}", quote_ident(pattern))
    } else {
        format!("-- Columns matching {}{place}", quote_ident(pattern))
    };
    let mut lines: Vec<String> = found
        .iter()
        .map(|column| {
            format!(
                "--   {}.{} {}",
                quote_ident(&column.table),
                quote_ident(&column.column),
                column.data_type
            )
        })
        .collect();
    if more {
        lines.push(format!("--   … more columns match (first {COLUMNS_LIMIT} shown); use a narrower pattern"));
    }
    Section { header, lines }
}

/// `definition:nome`: a view (ou view materializada) pelo catálogo; senão,
/// nos schemas do nome (ou do escopo), a função — todas as sobrecargas, ou a
/// da assinatura entre parênteses — e, por último, o índice.
async fn definitions_of(
    catalogs: &CatalogService,
    entry: &Arc<CatalogEntry>,
    db_type: DatabaseType,
    scope: &[String],
    flat: bool,
    target: &str,
) -> Result<Vec<Section>> {
    let (name, signature) = split_signature(target);
    if name.is_empty() {
        return Ok(Vec::new());
    }

    if let Some(Found::Relation(schema, relation)) = resolve(catalogs, entry, scope, &name, flat).await? {
        let kind = entry.read().find_relation(&schema, &relation).map(|(_, kind)| kind);
        let kind = match kind {
            Some(RelKind::View) => ObjectKind::View,
            Some(RelKind::MaterializedView) => ObjectKind::MaterializedView,
            // Tabela não tem definição aqui: a estrutura vem pelo nome sem prefixo
            _ => return Ok(Vec::new()),
        };
        let target = ObjectRef {
            kind,
            schema: schema.clone(),
            name: relation.clone(),
            table: None,
            signature: None,
        };
        let Some(text) = definition_text(catalogs, entry, db_type, &target).await? else {
            return Ok(Vec::new());
        };
        let label = if kind == ObjectKind::View {
            "view"
        } else {
            "materialized view"
        };
        return Ok(vec![definition_section(label, &schema, &relation, &text)]);
    }
    if db_type == DatabaseType::Mongodb {
        return Ok(Vec::new());
    }

    let (schemas, object) = match split_qualified(&name) {
        Some((schema, object)) if !flat => match resolve_schema(catalogs, entry, scope, &schema).await? {
            Some(schema) => (vec![schema], object),
            None => return Ok(Vec::new()),
        },
        _ if flat => (vec![FLAT_SCHEMA.to_string()], unquote(&name)),
        _ => (scope.to_vec(), unquote(&name)),
    };

    for schema in &schemas {
        let overloads: Vec<FunctionObject> = list_functions(catalogs, entry, schema)
            .await?
            .into_iter()
            .filter(|function| function.name == object || function.name.eq_ignore_ascii_case(&object))
            .filter(|function| {
                signature
                    .as_deref()
                    .is_none_or(|wanted| same_signature(wanted, &function.signature))
            })
            .take(MAX_OVERLOADS)
            .collect();
        if !overloads.is_empty() {
            let mut sections = Vec::new();
            for function in overloads {
                let target = ObjectRef {
                    kind: ObjectKind::Function,
                    schema: schema.clone(),
                    name: function.name.clone(),
                    table: None,
                    signature: Some(function.signature.clone()),
                };
                if let Some(text) = definition_text(catalogs, entry, db_type, &target).await? {
                    let label = format!("{}({})", function.name, function.signature);
                    sections.push(definition_section(
                        function_label(function.function_kind),
                        schema,
                        &label,
                        &text,
                    ));
                }
            }
            return Ok(sections);
        }

        let index = list_indexes(catalogs, entry, schema, None)
            .await?
            .into_iter()
            .find(|index| index.name == object || index.name.eq_ignore_ascii_case(&object));
        if let Some(index) = index {
            let target = ObjectRef {
                kind: ObjectKind::Index,
                schema: schema.clone(),
                name: index.name.clone(),
                table: None,
                signature: None,
            };
            if let Some(text) = definition_text(catalogs, entry, db_type, &target).await? {
                return Ok(vec![definition_section("index", schema, &index.name, &text)]);
            }
        }
    }
    Ok(Vec::new())
}

/// A definição pelo catálogo, com os literais trocados (`None` = não existe
/// ou o banco não tem definição para isso).
async fn definition_text(
    catalogs: &CatalogService,
    entry: &Arc<CatalogEntry>,
    db_type: DatabaseType,
    target: &ObjectRef,
) -> Result<Option<String>> {
    let text = match catalogs.definition(entry, target).await {
        Ok(text) => text,
        Err(Error::NotFound(_) | Error::UnsupportedType(_)) => return Ok(None),
        Err(error) => return Err(error),
    };
    Ok(Some(if db_type == DatabaseType::Mongodb {
        // `db.createView("nome", "origem", [pipeline])`: só o pipeline tem valores
        match text.find('[') {
            Some(at) => format!("{}{}", &text[..at], mask_json(&text[at..])),
            None => mask_json(&text),
        }
    } else {
        mask_sql(&text)
    }))
}

/// `app.add(integer, text)` → (`app.add`, `integer, text`).
fn split_signature(target: &str) -> (String, Option<String>) {
    let target = target.trim();
    match target.strip_suffix(')').and_then(|rest| rest.split_once('(')) {
        Some((name, signature)) => (name.trim().to_string(), Some(signature.trim().to_string())),
        None => (target.to_string(), None),
    }
}

/// A assinatura pedida casa com a da função: igual (sem espaços e
/// maiúsculas) ou só os tipos, na ordem (`integer, text` casa `a integer, b text`).
fn same_signature(wanted: &str, signature: &str) -> bool {
    let normalize = |text: &str| -> Vec<String> {
        text.split(',')
            .map(|arg| arg.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase())
            .filter(|arg| !arg.is_empty())
            .collect()
    };
    let (wanted, actual) = (normalize(wanted), normalize(signature));
    wanted.len() == actual.len()
        && wanted
            .iter()
            .zip(&actual)
            .all(|(wanted, actual)| actual == wanted || actual.ends_with(&format!(" {wanted}")))
}

fn function_label(kind: FunctionKind) -> &'static str {
    match kind {
        FunctionKind::Function => "function",
        FunctionKind::Procedure => "procedure",
        FunctionKind::Aggregate => "aggregate",
        FunctionKind::Window => "window function",
    }
}

fn indexes_section(schema: &str, table: Option<&str>, indexes: &[IndexObject]) -> Section {
    let header = match table {
        Some(table) if indexes.is_empty() => format!("-- No indexes on {}", schema_context::qualified(schema, table)),
        Some(table) => format!("-- Indexes on {}", schema_context::qualified(schema, table)),
        None if indexes.is_empty() => format!("-- No indexes in {}", quote_ident(schema)),
        None => format!("-- Indexes in {}", quote_ident(schema)),
    };
    let lines = indexes
        .iter()
        .map(|index| {
            let on = match table {
                Some(_) => String::new(),
                None => format!(" on {}", quote_ident(&index.table)),
            };
            let role = if index.primary {
                ", primary key"
            } else if index.unique {
                ", unique"
            } else {
                ""
            };
            let columns: Vec<String> = index.columns.iter().map(|column| mask_sql(column)).collect();
            let partial = index
                .predicate
                .as_deref()
                .map(|predicate| format!(" WHERE {}", mask_sql(predicate)))
                .unwrap_or_default();
            format!(
                "--   {}{on} {}{role} ({}){partial}",
                quote_ident(&index.name),
                index.method,
                columns.join(", ")
            )
        })
        .collect();
    Section { header, lines }
}

fn functions_section(schema: &str, functions: &[FunctionObject]) -> Section {
    let header = if functions.is_empty() {
        format!("-- No functions in {} (extensions not listed)", quote_ident(schema))
    } else {
        format!(
            "-- Functions in {} (extensions not listed; literals replaced by ?)",
            quote_ident(schema)
        )
    };
    let lines = functions
        .iter()
        .map(|function| {
            let returns = function
                .returns
                .as_deref()
                .map(|returns| format!(" returns {returns}"))
                .unwrap_or_default();
            let comment = function
                .comment
                .as_deref()
                .map(|text| format!(" -- {}", schema_context::comment(text)))
                .unwrap_or_default();
            format!(
                "--   {}({}){returns}, {}, {}{comment}",
                quote_ident(&function.name),
                mask_sql(&function.arguments),
                function_label(function.function_kind),
                function.language
            )
        })
        .collect();
    Section { header, lines }
}

fn definition_section(label: &str, schema: &str, name: &str, text: &str) -> Section {
    Section {
        header: format!(
            "-- Definition of {label} {} (literals replaced by ?)",
            schema_context::qualified(schema, name)
        ),
        lines: text.trim_end().lines().map(str::to_string).collect(),
    }
}

/// Os blocos no orçamento. Cada um fica com no máximo a sua parte do que
/// resta (uma definição longa não tira o lugar dos blocos seguintes), e o que
/// um bloco pequeno não usa passa para os próximos. Um bloco entra se cabem o
/// cabeçalho e a primeira linha; as linhas que não couberem viram um aviso.
/// Devolve o texto (se algum bloco entrou) e se algo ficou de fora.
fn render_sections(sections: &[Section], budget_chars: usize) -> (Option<String>, bool) {
    let mut out: Vec<String> = Vec::new();
    let mut used = 0;
    let mut truncated = false;
    for (position, section) in sections.iter().enumerate() {
        let remaining = budget_chars.saturating_sub(used);
        let share = remaining / (sections.len() - position);
        let header_cost = section.header.chars().count() + 1;
        let first_cost = section.lines.first().map_or(0, |line| line.chars().count() + 1);
        if header_cost + first_cost > remaining {
            truncated = true;
            continue;
        }
        // O mínimo do bloco cabe no que resta, mesmo passando da parte dele
        let cap = share.max(header_cost + first_cost);
        out.push(section.header.clone());
        let mut spent = header_cost;
        let mut left_out = 0;
        for (index, line) in section.lines.iter().enumerate() {
            let cost = line.chars().count() + 1;
            // Antes da última linha, guarda lugar para o aviso do corte
            let reserve = if index + 1 == section.lines.len() {
                0
            } else {
                NOTICE_RESERVE
            };
            if left_out == 0 && (index == 0 || spent + cost + reserve <= cap) {
                out.push(line.clone());
                spent += cost;
            } else {
                left_out += 1;
            }
        }
        if left_out > 0 {
            truncated = true;
            let notice = format!("-- … {left_out} more line(s) not shown");
            spent += notice.chars().count() + 1;
            out.push(notice);
        }
        used += spent;
    }
    ((!out.is_empty()).then(|| out.join("\n")), truncated)
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
    let schema_comments = schema_context::schema_comments(catalogs, entry, &schemas).await;
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
        schema_comments: &schema_comments,
        budget_tokens,
    }))
}

/// As coleções confirmadas, com o formato de uma amostra de cada.
async fn mongo_text(
    catalogs: &CatalogService,
    entry: &Arc<CatalogEntry>,
    relations: &[(String, String)],
    sample_values: bool,
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
    let shapes = catalogs.infer_shape(entry, &names, sample_values).await?;
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
    fn prefixes_pick_the_object_request() {
        assert_eq!(
            object_request("indexes:sales.orders"),
            Some(ObjectRequest::Indexes("sales.orders".into()))
        );
        assert_eq!(
            object_request(" Functions : app "),
            Some(ObjectRequest::Functions("app".into()))
        );
        assert_eq!(
            object_request("definition:app.add(integer, text)"),
            Some(ObjectRequest::Definition("app.add(integer, text)".into()))
        );
        // Sem prefixo conhecido, é nome comum (uma coleção pode ter dois-pontos)
        assert_eq!(object_request("audit:log"), None);
        assert_eq!(object_request("orders"), None);
    }

    #[test]
    fn signatures_match_with_or_without_argument_names() {
        assert_eq!(
            split_signature("app.add(integer, text)"),
            ("app.add".to_string(), Some("integer, text".to_string()))
        );
        assert_eq!(split_signature("app.add"), ("app.add".to_string(), None));
        assert!(same_signature("integer, text", "a integer, b text"));
        assert!(same_signature("A  Integer,b TEXT", "a integer, b text"));
        assert!(same_signature("", ""));
        assert!(!same_signature("integer", "a integer, b text"));
        assert!(!same_signature("text, integer", "a integer, b text"));
        assert!(same_signature("days integer", "IN days integer"));
    }

    #[test]
    fn sections_mask_literals_and_fit_the_budget() {
        let index = IndexObject {
            name: "orders_open".into(),
            table: "orders".into(),
            method: "btree".into(),
            unique: true,
            primary: false,
            columns: vec!["coalesce(status, 'CANARY')".into()],
            predicate: Some("status <> 'CANARY_void'::text".into()),
        };
        let section = indexes_section("app", None, std::slice::from_ref(&index));
        assert_eq!(section.header, "-- Indexes in \"app\"");
        assert_eq!(
            section.lines,
            ["--   \"orders_open\" on \"orders\" btree, unique (coalesce(status, ?)) WHERE status <> ?::text"]
        );
        assert_eq!(
            indexes_section("app", Some("orders"), &[]).header,
            "-- No indexes on \"app\".\"orders\""
        );

        let function = FunctionObject {
            name: "add".into(),
            signature: "a integer, b text".into(),
            arguments: "a integer, b text DEFAULT 'CANARY'::text".into(),
            returns: Some("text".into()),
            function_kind: FunctionKind::Function,
            language: "sql".into(),
            comment: Some("Soma\ndois «valores»".into()),
        };
        let section = functions_section("app", &[function]);
        assert_eq!(
            section.lines,
            ["--   \"add\"(a integer, b text DEFAULT ?::text) returns text, function, sql -- «Soma dois \"valores\"»"]
        );

        // Busca de colunas: o glob vira `LIKE` (e `%`, `_` e `\` valem por si)
        assert_eq!(like_pattern("*cpf*"), "%cpf%");
        assert_eq!(like_pattern("?d_x%\\"), "_d\\_x\\%\\\\");
        let found = [ColumnMatch {
            table: "Clientes".into(),
            column: "cpf".into(),
            data_type: "text".into(),
        }];
        let section = columns_section("app", "*cpf*", &found, true);
        assert_eq!(section.header, "-- Columns matching \"*cpf*\" in \"app\"");
        assert_eq!(section.lines[0], "--   \"Clientes\".\"cpf\" text");
        assert!(section.lines[1].contains("more columns match"), "{:?}", section.lines);
        assert_eq!(
            columns_section(FLAT_SCHEMA, "*cpf*", &[], false).header,
            "-- No columns matching \"*cpf*\""
        );

        let long = Section {
            header: "-- Definition of view \"app\".\"v\" (literals replaced by ?)".into(),
            lines: (0..100).map(|i| format!("  line {i:03} of the body")).collect(),
        };
        let small = Section {
            header: "-- Indexes on \"app\".\"t\"".into(),
            lines: vec!["--   \"t_pkey\" btree, primary key (id)".into()],
        };
        let (text, truncated) = render_sections(&[long, small], 600);
        let text = text.unwrap();
        assert!(truncated);
        assert!(text.chars().count() <= 600, "{text}");
        assert!(text.contains("more line(s) not shown"), "{text}");
        assert!(text.contains("t_pkey"), "o bloco seguinte ainda entra: {text}");

        // Nem o cabeçalho cabe: nada entra e é recorte
        assert_eq!(
            render_sections(
                &[Section {
                    header: "-- x".repeat(10),
                    lines: vec![]
                }],
                5
            ),
            (None, true)
        );
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

/// Os prefixos com fontes de verdade, pelo `CatalogService` do app: SQLite num
/// arquivo temporário (roda sempre) e Postgres/Mongo dos e2e (`--ignored`).
#[cfg(test)]
mod e2e {
    use super::*;
    use crate::models::{Server, SslMode};
    use crate::services::SourceOpener;
    use crate::storage::vault;

    fn server(db_type: DatabaseType, port: u16, connection_uri: Option<String>) -> Server {
        Server {
            id: Some(1),
            name: "e2e".into(),
            db_type,
            host: "localhost".into(),
            port,
            username: if db_type == DatabaseType::Postgres {
                "postgres".into()
            } else {
                String::new()
            },
            password: if db_type == DatabaseType::Postgres {
                "1234".into()
            } else {
                String::new()
            },
            default_database: None,
            ssl_mode: SslMode::Disable,
            ssl_root_cert: None,
            connection_uri,
            scope_databases: None,
            scope_schemas: None,
            group_id: None,
            assistant_sample_values: false,
            created_at: 0,
        }
    }

    async fn open(
        server: Server,
        database: &str,
        label: &str,
    ) -> (CatalogService, Arc<CatalogEntry>, std::path::PathBuf) {
        vault::init_for_tests();
        let dir = std::env::temp_dir().join(format!("octapus-requested-{label}-{}", std::process::id()));
        let service = CatalogService::default();
        service.init(dir.clone(), Arc::new(|_| {}));
        let db = database.to_string();
        let opener: SourceOpener = Arc::new(move || {
            let (server, db) = (server.clone(), db.clone());
            Box::pin(async move { crate::adapters::create_catalog_source(&server, &db).await })
        });
        let entry = service.open(1, database, label.into(), opener).await;
        (service, entry, dir)
    }

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|name| name.to_string()).collect()
    }

    #[tokio::test]
    async fn e2e_sqlite_indexes_and_definitions_without_literals() {
        let path = std::env::temp_dir().join("octapus_db_requested_objects.db");
        let _ = std::fs::remove_file(&path);
        rusqlite::Connection::open(&path)
            .unwrap()
            .execute_batch(
                "CREATE TABLE orders (id INTEGER PRIMARY KEY, status TEXT UNIQUE, total REAL);
                 CREATE INDEX orders_open ON orders(total) WHERE status = 'CANARY_open';
                 CREATE VIEW big_orders AS SELECT * FROM orders WHERE total > 4242 AND status <> 'CANARY_void';",
            )
            .unwrap();
        let server = server(DatabaseType::Sqlite, 0, Some(path.to_string_lossy().into_owned()));
        let (service, entry, dir) = open(server, "main", "sqlite").await;
        let scope = [FLAT_SCHEMA.to_string()];

        let structure = from_catalog(
            &service,
            &entry,
            &Lookup::new(DatabaseType::Sqlite, &scope),
            &names(&[
                "orders",
                "indexes:orders",
                "definition:big_orders",
                "definition:orders_open",
                "functions:main",
                "definition:ghost",
                "columns:*TOT*",
                "columns:orders",
                "columns:*",
                "columns:*nada*",
            ]),
            4_000,
        )
        .await
        .unwrap();
        let text = &structure.text;
        assert!(!text.contains("CANARY") && !text.contains("4242"), "{text}");
        assert!(text.contains("CREATE TABLE \"orders\""), "{text}");
        // A busca não diferencia maiúsculas e passa pela view também
        assert!(
            text.contains("-- Columns matching \"*TOT*\"\n--   \"big_orders\".\"total\" REAL\n--   \"orders\".\"total\" REAL"),
            "{text}"
        );
        assert!(text.contains("-- No columns matching \"*nada*\""), "{text}");
        // `columns:orders` é a própria tabela: não repete o DDL
        assert_eq!(text.matches("CREATE TABLE \"orders\"").count(), 1, "{text}");
        assert!(
            text.contains("-- Indexes on \"orders\"\n--   \"orders_open\" btree (total) WHERE status = ?"),
            "{text}"
        );
        // O autoindex do UNIQUE fica de fora
        assert!(!text.contains("sqlite_autoindex"), "{text}");
        assert!(
            text.contains("-- Definition of view \"big_orders\" (literals replaced by ?)\nCREATE VIEW big_orders AS SELECT * FROM orders WHERE total > ? AND status <> ?;"),
            "{text}"
        );
        assert!(
            text.contains("CREATE INDEX orders_open ON orders(total) WHERE status = ?;"),
            "{text}"
        );
        assert_eq!(
            structure.missing,
            names(&["functions:main", "definition:ghost", "columns:*"]),
            "SQLite não tem funções; só curinga não é busca"
        );
        assert_eq!((structure.requested, structure.found), (10, 7));
        assert!(!structure.truncated);

        // Orçamento curto: corta e avisa
        let tight = from_catalog(
            &service,
            &entry,
            &Lookup::new(DatabaseType::Sqlite, &scope),
            &names(&["definition:big_orders", "indexes:*"]),
            30,
        )
        .await
        .unwrap();
        assert!(tight.truncated, "{}", tight.text);

        drop(entry);
        let _ = std::fs::remove_dir_all(dir);
        let _ = std::fs::remove_file(path);
    }

    /// `cargo test --lib assistant::requested::e2e -- --ignored`
    #[tokio::test]
    #[ignore = "requires local postgres on localhost:5432 (postgres/1234)"]
    async fn e2e_postgres_indexes_functions_and_definitions() {
        use crate::adapters::postgres::PostgresAdapter;

        const DATABASE: &str = "octapus_db_e2e_requested_objects";
        let pg = server(DatabaseType::Postgres, 5432, None);
        let admin = PostgresAdapter::new(&pg, "postgres").unwrap();
        admin
            .execute_statement(&format!("DROP DATABASE IF EXISTS {DATABASE} WITH (FORCE)"))
            .await
            .unwrap();
        admin
            .execute_statement(&format!("CREATE DATABASE {DATABASE}"))
            .await
            .unwrap();
        let setup = PostgresAdapter::new(&pg, DATABASE).unwrap();
        setup
            .execute_transaction(
                [
                    "CREATE SCHEMA app",
                    "CREATE TABLE app.orders (id serial PRIMARY KEY, status text NOT NULL, total numeric(10,2))",
                    "CREATE INDEX orders_open ON app.orders (total) WHERE status = 'CANARY_open'",
                    "CREATE VIEW app.paid AS SELECT * FROM app.orders WHERE status = 'CANARY_paid' AND total > 4242",
                    "CREATE FUNCTION app.add(a integer, b integer DEFAULT 77) RETURNS integer LANGUAGE sql AS 'SELECT a + b'",
                    "CREATE FUNCTION app.add(a text, b text) RETURNS text LANGUAGE plpgsql AS $$BEGIN RETURN a || 'CANARY_sep' || b; END$$",
                    "CREATE EXTENSION IF NOT EXISTS pgcrypto SCHEMA app",
                    "COMMENT ON SCHEMA app IS 'Aplicação da loja'",
                    "COMMENT ON FUNCTION app.add(integer, integer) IS 'Soma dois inteiros'",
                ]
                .map(String::from)
                .to_vec(),
            )
            .await
            .unwrap();
        drop(setup);

        let (service, entry, dir) = open(pg.clone(), DATABASE, "postgres").await;
        let scope = ["app".to_string()];
        let structure = from_catalog(
            &service,
            &entry,
            &Lookup::new(DatabaseType::Postgres, &scope),
            &names(&[
                "indexes:app.orders",
                "functions:app",
                "definition:app.paid",
                "definition:app.add(text, text)",
                "definition:orders_open",
                "definition:app.orders",
                "functions:ghost",
                "app.orders",
                "columns:app.*TOT*",
                "columns:*stat*",
                "columns:ghost.*x*",
            ]),
            8_000,
        )
        .await
        .unwrap();
        let text = &structure.text;
        assert!(
            !text.contains("CANARY") && !text.contains("4242") && !text.contains("77"),
            "{text}"
        );
        assert!(
            text.contains("--   \"orders_open\" btree (total) WHERE status = ?::text"),
            "{text}"
        );
        assert!(text.contains("--   \"orders_pkey\" btree, primary key (id)"), "{text}");
        assert!(
            text.contains(
                "--   \"add\"(a integer, b integer DEFAULT ?) returns integer, function, sql -- «Soma dois inteiros»"
            ),
            "{text}"
        );
        assert!(
            text.starts_with("-- PostgreSQL. Schemas: \"app\"\n-- Schema \"app\": «Aplicação da loja»\n"),
            "{text}"
        );
        assert!(
            text.contains(
                "-- Columns matching \"*TOT*\" in \"app\"\n--   \"orders\".\"total\" numeric(10,2)\n--   \"paid\".\"total\" numeric(10,2)"
            ),
            "{text}"
        );
        assert!(text.contains("-- Columns matching \"*stat*\" in \"app\"\n--   \"orders\".\"status\" text"), "{text}");
        assert!(!text.contains("digest"), "funções do pgcrypto ficam de fora: {text}");
        assert!(text.contains("-- Definition of view \"app\".\"paid\""), "{text}");
        assert!(
            text.contains("-- Definition of function \"app\".\"add(a text, b text)\""),
            "{text}"
        );
        assert!(text.contains("RETURN a || ? || b;"), "{text}");
        assert!(
            text.contains("CREATE INDEX orders_open ON app.orders USING btree (total) WHERE (status = ?::text);"),
            "{text}"
        );
        // Tabela não tem `definition:`; schema inexistente
        assert_eq!(
            structure.missing,
            names(&["definition:app.orders", "functions:ghost", "columns:ghost.*x*"])
        );

        drop(entry);
        drop(service);
        let _ = std::fs::remove_dir_all(dir);
        admin
            .execute_statement(&format!("DROP DATABASE IF EXISTS {DATABASE} WITH (FORCE)"))
            .await
            .unwrap();
    }

    /// Mongo sem auth em localhost:27017 (ou `OCTAPUS_E2E_MONGO_PORT`).
    /// `cargo test --lib assistant::requested::e2e -- --ignored`
    #[tokio::test]
    #[ignore = "requires local mongodb without auth (OCTAPUS_E2E_MONGO_PORT, default 27017)"]
    async fn e2e_mongo_indexes_and_view_pipeline_without_values() {
        use mongodb::bson::{doc, Document};

        const DATABASE: &str = "octapus_e2e_requested_objects";
        let port: u16 = std::env::var("OCTAPUS_E2E_MONGO_PORT")
            .ok()
            .and_then(|port| port.parse().ok())
            .unwrap_or(27017);
        let client = mongodb::Client::with_uri_str(format!("mongodb://localhost:{port}"))
            .await
            .unwrap();
        let db = client.database(DATABASE);
        db.drop().await.unwrap();
        let orders = db.collection::<Document>("orders");
        orders
            .insert_one(doc! { "status": "CANARY_paid", "total": 4242 })
            .await
            .unwrap();
        orders
            .create_index(mongodb::IndexModel::builder().keys(doc! { "status": 1 }).build())
            .await
            .unwrap();
        db.create_collection("paid_orders")
            .view_on("orders".to_string())
            .pipeline(vec![
                doc! { "$match": { "status": "CANARY_paid", "total": { "$gt": 4242 } } },
            ])
            .await
            .unwrap();

        // Coleção com validador: as descrições vão sempre; os valores, só com o
        // interruptor do servidor
        db.create_collection("tickets")
            .validator(doc! { "$jsonSchema": {
                "description": "Chamados do suporte",
                "properties": {
                    "status": { "description": "Situação do chamado" },
                    "items": { "items": { "properties": { "sku": { "description": "Código do item" } } } },
                },
            } })
            .await
            .unwrap();
        let tickets: Vec<Document> = (0..30)
            .map(|i| {
                doc! {
                    "status": if i % 3 == 0 { "aberto" } else { "pago" },
                    "customerName": if i % 2 == 0 { "CANARY_ana" } else { "CANARY_bia" },
                    "items": [{ "sku": "A1" }],
                }
            })
            .collect();
        db.collection::<Document>("tickets").insert_many(tickets).await.unwrap();

        let (service, entry, dir) = open(server(DatabaseType::Mongodb, port, None), DATABASE, "mongo").await;
        let scope = [FLAT_SCHEMA.to_string()];
        let structure = from_catalog(
            &service,
            &entry,
            &Lookup::new(DatabaseType::Mongodb, &scope),
            &names(&[
                "indexes:orders",
                "indexes:paid_orders",
                "definition:paid_orders",
                "functions:x",
                "indexes:*",
            ]),
            4_000,
        )
        .await
        .unwrap();
        let text = &structure.text;
        assert!(!text.contains("CANARY") && !text.contains("4242"), "{text}");
        assert!(text.contains("-- Indexes on \"orders\"\n--   \"_id_\" regular, primary key (_id)\n--   \"status_1\" regular (status)"), "{text}");
        assert!(text.contains("-- No indexes on \"paid_orders\""), "{text}");
        assert!(text.contains("db.createView(\"paid_orders\", \"orders\", ["), "{text}");
        assert!(text.contains("\"status\": ?") && text.contains("\"$gt\": ?"), "{text}");
        assert_eq!(structure.missing, names(&["functions:x", "indexes:*"]));

        let lookup = |sample_values| Lookup {
            db_type: DatabaseType::Mongodb,
            scope: &scope,
            sample_values,
        };
        let off = from_catalog(&service, &entry, &lookup(false), &names(&["tickets", "columns:*sta*"]), 4_000)
            .await
            .unwrap();
        assert!(
            off.text.contains("collection \"tickets\" (30 sampled documents): -- «Chamados do suporte»"),
            "{}",
            off.text
        );
        assert!(off.text.contains("\n  \"status\" string 100% -- «Situação do chamado»"), "{}", off.text);
        assert!(off.text.contains("\n  \"items[].sku\" string 100% -- «Código do item»"), "{}", off.text);
        assert!(!off.text.contains("values:") && !off.text.contains("pago"), "{}", off.text);
        // O Mongo não tem busca de colunas
        assert_eq!(off.missing, names(&["columns:*sta*"]));

        let on = from_catalog(&service, &entry, &lookup(true), &names(&["tickets"]), 4_000).await.unwrap();
        assert!(
            on.text.contains("\n  \"status\" string 100% values: \"pago\" 67%, \"aberto\" 33% -- «Situação do chamado»"),
            "{}",
            on.text
        );
        // Nome de pessoa nunca mostra valor
        assert!(!on.text.contains("CANARY"), "{}", on.text);

        drop(entry);
        drop(service);
        let _ = std::fs::remove_dir_all(dir);
        db.drop().await.unwrap();
    }
}
