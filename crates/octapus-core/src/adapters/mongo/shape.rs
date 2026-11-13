//! Formato de uma coleção a partir de uma amostra de documentos, para o
//! contexto do assistente (B8). É o `list_columns` da árvore com outra
//! finalidade: caminhos aninhados (`customer.address.city`, `items[].sku`),
//! todos os tipos vistos e a % de presença.
//!
//! **Os valores ficam aqui** (RN-08): a saída tem só caminhos, nomes de tipo
//! BSON e porcentagens. E como no Mongo uma chave pode ser dado (um mapa
//! `{"ana@x.com": 3}`), a chave que parece valor vira `{key}`:
//! - pelo formato: número, UUID, ObjectId/hex, e-mail, data, texto longo;
//! - pelo uso: um objeto com muitas chaves diferentes, cada uma em poucos
//!   documentos, é um mapa — todas as chaves dele viram `{key}`.
//!
//! A exceção, só com o servidor ligando (`Server::assistant_sample_values`):
//! os valores de exemplo de um campo de texto com cara de enum (`status`,
//! `channel`), com a fatia de cada um — sem eles o modelo chuta o `"paid"`.
//! Um campo só entra se a amostra o mostrar repetindo poucos valores curtos
//! ([`categorical`]); nome de pessoa, e-mail, documento e texto livre ficam de
//! fora pelo nome do campo e pelo formato do valor.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use mongodb::bson::{Bson, Document};

use crate::catalog::{CollectionShape, FieldShape, ValueShare};

use super::types::bson_type_name;

/// Documentos por amostra (`$sample`).
pub const SAMPLE_SIZE: i64 = 100;
/// Campos por coleção, no máximo (ficam os mais presentes).
pub const MAX_FIELDS: usize = 120;
/// Níveis de caminho, no máximo; abaixo disso só o tipo.
const MAX_DEPTH: usize = 6;
/// Um objeto com mais chaves diferentes que isto, cada uma em menos da metade
/// dos documentos que o têm, é um mapa.
const MAP_MIN_KEYS: usize = 20;
/// Chave mais longa que isto é dado, não nome de campo.
const KEY_MAX_CHARS: usize = 64;
/// O que entra no lugar de uma chave que é dado.
pub const KEY_PLACEHOLDER: &str = "{key}";

/// Valores de exemplo por campo, no máximo (os mais comuns).
const MAX_VALUES: usize = 8;
/// Valores distintos acompanhados por campo; passou disso, não é enum.
const MAX_DISTINCT: usize = 24;
/// Ocorrências de texto que um campo precisa ter na amostra para mostrar
/// valores (com poucas, um valor pode ser de uma pessoa só).
const VALUES_MIN_OCCURRENCES: usize = 10;
/// Em média, cada valor distinto se repete pelo menos isto.
const VALUES_MIN_REPEAT: usize = 4;
/// Valor mais longo que isto é texto livre, não categoria.
const VALUE_MAX_CHARS: usize = 40;
/// Campos cujo nome diz que o valor é de uma pessoa ou segredo: nunca mostram
/// valor, mesmo repetindo. Comparado com cada palavra do último segmento do
/// caminho (`customerName` → `customer`, `name`).
const SENSITIVE_WORDS: &[&str] = &[
    "name", "nome", "email", "mail", "phone", "fone", "telefone", "celular", "mobile", "cpf", "cnpj", "rg",
    "ssn", "document", "documento", "passport", "password", "senha", "pass", "pwd", "token", "secret", "hash",
    "key", "address", "endereco", "street", "rua", "cep", "zip", "postal", "ip", "birth", "birthday",
    "nascimento", "card", "cartao", "iban", "account", "conta", "salary", "salario", "username",
    "login", "comment", "note", "notes", "description", "bio", "text", "message", "url",
];

/// O formato pela amostra. Os documentos não saem daqui; com `values`, os
/// valores de exemplo dos campos categóricos saem no formato.
pub fn infer(collection: &str, docs: &[Document], values: bool) -> CollectionShape {
    let maps = map_parents(docs);

    // Por caminho: em quantos documentos aparece e quantas vezes cada tipo
    let mut presence: HashMap<String, usize> = HashMap::new();
    let mut types: HashMap<String, HashMap<&'static str, usize>> = HashMap::new();
    let mut texts: HashMap<String, TextCounts> = HashMap::new();
    for doc in docs {
        let mut seen: HashSet<String> = HashSet::new();
        walk_document(doc, "", 0, &maps, &mut |event| {
            *types.entry(event.path.to_string()).or_default().entry(bson_type_name(event.value)).or_default() += 1;
            if seen.insert(event.path.to_string()) {
                *presence.entry(event.path.to_string()).or_default() += 1;
            }
            if let (true, Bson::String(text)) = (values, event.value) {
                texts.entry(event.path.to_string()).or_default().add(text);
            }
        });
    }

    let sampled = docs.len();
    let mut fields: Vec<FieldShape> = presence
        .into_iter()
        .map(|(path, present)| {
            let mut counts: Vec<(&'static str, usize)> = types.remove(&path).unwrap_or_default().into_iter().collect();
            counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
            let (values, more_values) = texts
                .remove(&path)
                .and_then(|counts| categorical(&path, counts))
                .unwrap_or_default();
            FieldShape {
                presence: percent(present, sampled),
                types: counts.into_iter().map(|(name, _)| name).collect(),
                path,
                values,
                more_values,
                description: None,
            }
        })
        .collect();

    let truncated = fields.len() > MAX_FIELDS;
    if truncated {
        fields.sort_by(|a, b| b.presence.cmp(&a.presence).then_with(|| path_order(&a.path, &b.path)));
        fields.truncate(MAX_FIELDS);
    }
    fields.sort_by(|a, b| path_order(&a.path, &b.path));

    CollectionShape {
        collection: collection.to_string(),
        sampled,
        fields,
        truncated,
        description: None,
    }
}

/// Os textos de um caminho na amostra: quantos, e quantas vezes cada um até
/// [`MAX_DISTINCT`] (depois disso só conta, e o campo já não é enum).
#[derive(Default)]
struct TextCounts {
    occurrences: usize,
    counts: HashMap<String, usize>,
    overflow: bool,
}

impl TextCounts {
    fn add(&mut self, text: &str) {
        self.occurrences += 1;
        if self.overflow {
            return;
        }
        if let Some(count) = self.counts.get_mut(text) {
            *count += 1;
        } else if self.counts.len() < MAX_DISTINCT {
            self.counts.insert(text.to_string(), 1);
        } else {
            self.overflow = true;
            self.counts.clear();
        }
    }
}

/// Os valores de exemplo de um campo com cara de enum, com quantos distintos
/// ficaram de fora; `None` quando o campo não é categórico:
/// - nome de pessoa, contato, documento ou segredo ([`SENSITIVE_WORDS`]);
/// - poucas ocorrências, valores demais ou que pouco se repetem;
/// - algum valor longo, com quebra de linha ou com cara de dado (número,
///   e-mail, id, data — [`looks_like_data`]).
///
/// Valor visto uma vez só nunca aparece: entra na conta dos de fora.
fn categorical(path: &str, texts: TextCounts) -> Option<(Vec<ValueShare>, usize)> {
    let last = path.rsplit('.').next().unwrap_or(path).trim_end_matches("[]");
    if texts.overflow
        || texts.occurrences < VALUES_MIN_OCCURRENCES
        || texts.counts.len() * VALUES_MIN_REPEAT > texts.occurrences
        || words(last).iter().any(|word| SENSITIVE_WORDS.contains(&word.as_str()))
    {
        return None;
    }
    let clean = |value: &str| {
        !value.trim().is_empty()
            && value.chars().count() <= VALUE_MAX_CHARS
            && !value.chars().any(char::is_control)
            && !looks_like_data(value)
    };
    if !texts.counts.keys().all(|value| clean(value)) {
        return None;
    }

    let distinct = texts.counts.len();
    let mut counts: Vec<(String, usize)> = texts.counts.into_iter().filter(|(_, count)| *count > 1).collect();
    counts.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    counts.truncate(MAX_VALUES);
    let shown = counts.len();
    let values = counts
        .into_iter()
        .map(|(value, count)| ValueShare {
            value,
            percent: percent(count, texts.occurrences),
        })
        .collect();
    Some((values, distinct - shown))
}

/// As palavras de um nome de campo, em minúsculas: `customerName`,
/// `customer_name` e `customer-name` → `customer`, `name`.
fn words(name: &str) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut previous_lower = false;
    for c in name.chars() {
        if !c.is_alphanumeric() {
            if !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
            previous_lower = false;
            continue;
        }
        if c.is_uppercase() && previous_lower && !current.is_empty() {
            words.push(std::mem::take(&mut current));
        }
        previous_lower = c.is_lowercase() || c.is_ascii_digit();
        current.extend(c.to_lowercase());
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

/// `_id` primeiro; depois por nível (`a`, `a.b`, `a[]`, `a[].c`, `b`).
fn path_order(a: &str, b: &str) -> Ordering {
    let is_id = |path: &str| path == "_id";
    is_id(b)
        .cmp(&is_id(a))
        .then_with(|| a.split('.').cmp(b.split('.')))
}

/// Presença em %, arredondada, nunca 0 para um campo que apareceu.
fn percent(present: usize, total: usize) -> u8 {
    if total == 0 {
        return 0;
    }
    ((present * 100 + total / 2) / total).clamp(1, 100) as u8
}

/// A chave parece um valor, não um nome de campo?
pub fn looks_like_data(key: &str) -> bool {
    let chars = key.chars().count();
    if chars == 0 || chars > KEY_MAX_CHARS {
        return true;
    }
    let is_hex = |text: &str| text.chars().all(|c| c.is_ascii_hexdigit());
    let digits = key.chars().filter(char::is_ascii_digit).count();
    key.chars().all(|c| c.is_ascii_digit())
        || key.contains('@')
        || is_uuid(key)
        // ObjectId e hashes: hex comprido com algum dígito
        || (chars >= 16 && is_hex(key) && digits > 0)
        // Data no começo (2026-10-20, 2026-10-20T…)
        || is_date_prefix(key)
}

fn is_uuid(key: &str) -> bool {
    let parts: Vec<&str> = key.split('-').collect();
    parts.len() == 5
        && parts.iter().map(|part| part.len()).eq([8, 4, 4, 4, 12])
        && parts.iter().all(|part| part.chars().all(|c| c.is_ascii_hexdigit()))
}

fn is_date_prefix(key: &str) -> bool {
    let bytes = key.as_bytes();
    bytes.len() >= 10
        && bytes[..4].iter().all(u8::is_ascii_digit)
        && bytes[4] == b'-'
        && bytes[5..7].iter().all(u8::is_ascii_digit)
        && bytes[7] == b'-'
        && bytes[8..10].iter().all(u8::is_ascii_digit)
}

struct Event<'a> {
    /// O caminho do objeto onde a chave está (`""` no documento)
    parent: &'a str,
    /// A chave como veio (`None` para elemento de array)
    key: Option<&'a str>,
    path: &'a str,
    value: &'a Bson,
}

fn walk_document(doc: &Document, parent: &str, depth: usize, maps: &HashSet<String>, f: &mut dyn FnMut(Event<'_>)) {
    for (key, value) in doc {
        let segment = if maps.contains(parent) || looks_like_data(key) {
            KEY_PLACEHOLDER
        } else {
            key.as_str()
        };
        let path = if parent.is_empty() {
            segment.to_string()
        } else {
            format!("{parent}.{segment}")
        };
        f(Event {
            parent,
            key: Some(key),
            path: &path,
            value,
        });
        walk_value(value, &path, depth + 1, maps, f);
    }
}

fn walk_value(value: &Bson, path: &str, depth: usize, maps: &HashSet<String>, f: &mut dyn FnMut(Event<'_>)) {
    if depth >= MAX_DEPTH {
        return;
    }
    match value {
        Bson::Document(doc) => walk_document(doc, path, depth, maps, f),
        Bson::Array(items) => {
            let element = format!("{path}[]");
            for item in items {
                f(Event {
                    parent: path,
                    key: None,
                    path: &element,
                    value: item,
                });
                walk_value(item, &element, depth + 1, maps, f);
            }
        }
        _ => {}
    }
}

/// Os objetos que são mapas (ver o topo). Um mapa dentro de um mapa só
/// aparece depois que o de fora vira `{key}`, então repete até estabilizar.
fn map_parents(docs: &[Document]) -> HashSet<String> {
    let mut maps: HashSet<String> = HashSet::new();
    for _ in 0..=MAX_DEPTH {
        // Por objeto: em quantos documentos aparece, e em quantos cada chave
        let mut parents: HashMap<String, (usize, HashMap<String, usize>)> = HashMap::new();
        for doc in docs {
            let mut seen: HashSet<(String, String)> = HashSet::new();
            let mut parents_seen: HashSet<String> = HashSet::new();
            walk_document(doc, "", 0, &maps, &mut |event| {
                let Some(key) = event.key else {
                    return;
                };
                if parents_seen.insert(event.parent.to_string()) {
                    parents.entry(event.parent.to_string()).or_default().0 += 1;
                }
                if seen.insert((event.parent.to_string(), key.to_string())) {
                    *parents.entry(event.parent.to_string()).or_default().1.entry(key.to_string()).or_default() += 1;
                }
            });
        }

        let found: Vec<String> = parents
            .into_iter()
            .filter(|(parent, (docs_with_parent, keys))| {
                if maps.contains(parent) || keys.len() <= MAP_MIN_KEYS {
                    return false;
                }
                let occurrences: usize = keys.values().sum();
                // Em média, cada chave em menos da metade dos documentos
                occurrences * 2 < docs_with_parent * keys.len()
            })
            .map(|(parent, _)| parent)
            .collect();
        if found.is_empty() {
            break;
        }
        maps.extend(found);
    }
    maps
}

#[cfg(test)]
mod tests {
    use super::*;
    use mongodb::bson::{doc, oid::ObjectId, DateTime};

    fn field<'a>(shape: &'a CollectionShape, path: &str) -> &'a FieldShape {
        shape
            .fields
            .iter()
            .find(|field| field.path == path)
            .unwrap_or_else(|| panic!("sem {path}: {:?}", shape.fields))
    }

    fn orders() -> Vec<Document> {
        (0..10)
            .map(|i| {
                let mut doc = doc! {
                    "_id": ObjectId::new(),
                    "status": if i % 2 == 0 { "paid" } else { "open" },
                    "total": if i < 8 { Bson::Double(10.5) } else { Bson::Int32(3) },
                    "customer": { "name": "Ana", "address": { "city": "Porto Alegre" } },
                    "items": [ { "sku": "A1", "qty": 2 }, { "sku": "B2" } ],
                    "tags": ["x", "y"],
                    "createdAt": DateTime::now(),
                };
                if i < 3 {
                    doc.insert("coupon", Bson::Null);
                }
                doc
            })
            .collect()
    }

    #[test]
    fn infers_nested_paths_types_and_presence() {
        let shape = infer("orders", &orders(), false);
        assert_eq!(shape.sampled, 10);
        assert!(!shape.truncated);
        assert_eq!(shape.fields[0].path, "_id");
        assert_eq!(field(&shape, "_id").types, ["objectId"]);
        assert_eq!(field(&shape, "total").types, ["double", "int"]);
        assert_eq!(field(&shape, "customer").types, ["object"]);
        assert_eq!(field(&shape, "customer.address.city").types, ["string"]);
        assert_eq!(field(&shape, "items[]").types, ["object"]);
        assert_eq!(field(&shape, "items[].sku").presence, 100);
        assert_eq!(field(&shape, "items[].qty").types, ["int"]);
        assert_eq!(field(&shape, "tags[]").types, ["string"]);
        assert_eq!(field(&shape, "createdAt").types, ["date"]);
        let coupon = field(&shape, "coupon");
        assert_eq!((coupon.types.as_slice(), coupon.presence), (["null"].as_slice(), 30));

        // Ordem estável: pai antes dos filhos, objeto antes dos elementos
        let paths: Vec<&str> = shape.fields.iter().map(|field| field.path.as_str()).collect();
        let position = |path: &str| paths.iter().position(|p| *p == path).unwrap();
        assert!(position("customer") < position("customer.address"));
        assert!(position("items") < position("items[]"));
        assert!(position("items[]") < position("items[].qty"));
        assert_eq!(infer("orders", &orders(), false).fields, shape.fields);
    }

    #[test]
    fn empty_sample_has_no_fields() {
        let shape = infer("empty", &[], false);
        assert_eq!((shape.sampled, shape.fields.len()), (0, 0));
    }

    #[test]
    fn data_shaped_keys_are_generalized() {
        for key in [
            "12345",
            "ana@exemplo.com",
            "550e8400-e29b-41d4-a716-446655440000",
            "507f1f77bcf86cd799439011",
            "2026-10-20",
            "2026-10-20T10:00:00Z",
            &"x".repeat(80),
        ] {
            assert!(looks_like_data(key), "{key}");
        }
        for key in ["_id", "name", "createdAt", "address_line_2", "deadbeef", "v2", "First Name"] {
            assert!(!looks_like_data(key), "{key}");
        }
    }

    #[test]
    fn wide_documents_are_not_maps() {
        // 30 campos em todos os documentos: estrutura, não mapa
        let docs: Vec<Document> = (0..5)
            .map(|_| (0..30).map(|i| (format!("field_{i}"), Bson::Int32(i))).collect())
            .collect();
        let shape = infer("wide", &docs, false);
        assert!(shape.fields.iter().any(|field| field.path == "field_29"));
        assert!(!shape.fields.iter().any(|field| field.path.contains(KEY_PLACEHOLDER)));
    }

    /// RN-08: nenhuma string de valor sai da inferência — nem em valor, nem
    /// em chave que é dado (mapas, e-mails, ids).
    #[test]
    fn canary_values_never_leave() {
        let docs: Vec<Document> = (0..30)
            .map(|i| {
                doc! {
                    "_id": format!("CANARY_id_{i}"),
                    "name": "CANARY_7f3a",
                    "notes": ["CANARY_9b1c", { "text": "CANARY_aa01" }],
                    "profile": { "bio": "CANARY_bb02", "since": "2026-13-45" },
                    // Mapa: uma chave diferente por documento
                    "scores": { format!("CANARY_user_{i}"): i, "CANARY_shared": 1 },
                    // Chaves com cara de dado
                    "byEmail": { format!("canary{i}@exemplo.com"): true },
                    "byId": { "507f1f77bcf86cd799439011": { "count": 1 } },
                    "byDay": { "2026-10-20": 5 },
                    "deep": { "a": { "b": { "c": { "d": { "e": { "f": { "g": "CANARY_cc03" } } } } } } },
                }
            })
            .collect();
        // Com os valores de exemplo ligados, os canários seguem fora: valor
        // distinto por documento, campo de pessoa ou texto, fundo demais
        let shape = infer("people", &docs, true);
        let text = format!("{shape:?}");
        for canary in ["CANARY", "canary", "exemplo.com", "2026-13-45", "507f1f77", "2026-10-20"] {
            assert!(!text.contains(canary), "{canary} vazou: {text}");
        }
        assert!(field(&shape, "scores").types == ["object"]);
        assert_eq!(field(&shape, "scores.{key}").types, ["int"]);
        assert_eq!(field(&shape, "byEmail.{key}").types, ["bool"]);
        assert_eq!(field(&shape, "byId.{key}.count").types, ["int"]);
    }

    fn ticket(i: usize) -> Document {
        let status = match i % 10 {
            0..=5 => "paid",
            6..=8 => "open",
            _ => "void",
        };
        let even = i.is_multiple_of(2);
        let mut doc = doc! {
            "status": status,
            "channel": if even { "web" } else { "app" },
            // Valor de pessoa, mesmo repetindo pouco
            "customerName": if even { "Ana Souza" } else { "Bia Lima" },
            "contact": { "email": "ana@exemplo.com" },
            // Número em texto: dado, não categoria
            "code": if even { "1001" } else { "1002" },
            // Valor distinto por documento
            "ref": format!("R-{i}"),
            "items": [ { "kind": "book" }, { "kind": "pen" } ],
        };
        if i == 0 {
            doc.insert("status", "refunded");
        }
        doc
    }

    #[test]
    fn categorical_values_only_when_asked() {
        let docs: Vec<Document> = (0..40).map(ticket).collect();
        let off = infer("tickets", &docs, false);
        assert!(off.fields.iter().all(|field| field.values.is_empty() && field.more_values == 0));

        let shape = infer("tickets", &docs, true);
        let status = field(&shape, "status");
        let values: Vec<(&str, u8)> = status.values.iter().map(|share| (share.value.as_str(), share.percent)).collect();
        // `refunded` apareceu uma vez só: fica na conta dos de fora
        assert_eq!(values, [("paid", 58), ("open", 30), ("void", 10)]);
        assert_eq!(status.more_values, 1);
        assert_eq!(field(&shape, "channel").values.len(), 2);
        assert_eq!(field(&shape, "items[].kind").values.len(), 2);
        for path in ["customerName", "contact.email", "code", "ref", "_id"] {
            if let Some(found) = shape.fields.iter().find(|field| field.path == path) {
                assert!(found.values.is_empty(), "{path}: {:?}", found.values);
            }
        }

        // Amostra curta: poucas ocorrências, nenhum valor
        let few = infer("tickets", &docs[..6], true);
        assert!(field(&few, "status").values.is_empty());
    }

    #[test]
    fn splits_field_names_into_words() {
        assert_eq!(words("customerName"), ["customer", "name"]);
        assert_eq!(words("customer_name"), ["customer", "name"]);
        assert_eq!(words("billing-address2"), ["billing", "address2"]);
        assert_eq!(words("status"), ["status"]);
        assert_eq!(words("{key}"), ["key"]);
        assert_eq!(words("CPF"), ["cpf"]);
    }

    #[test]
    fn many_fields_keep_the_most_present() {
        let docs: Vec<Document> = (0..2)
            .map(|d| {
                (0..(MAX_FIELDS + 30))
                    .filter(|i| d == 0 || *i < 10)
                    .map(|i| (format!("f{i:03}"), Bson::Int32(1)))
                    .collect()
            })
            .collect();
        let shape = infer("wide", &docs, false);
        assert!(shape.truncated);
        assert_eq!(shape.fields.len(), MAX_FIELDS);
        assert!(shape.fields.iter().any(|field| field.path == "f009" && field.presence == 100));
    }
}
