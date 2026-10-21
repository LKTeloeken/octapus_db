//! A operação de leitura que o assistente gera para o Mongo: o JSON do
//! `mongo-operation.schema.json` do contrato (`src-tauri/contract/`), com os
//! valores em Extended JSON relaxado.
//!
//! O parse é estrito (falha fechado): campo desconhecido, tipo errado ou campo
//! que não serve para a operação (`pipeline` num `find`) é recusado. O guard
//! (`assistant/guard/mongo.rs`) e o executor só-leitura (`read_only.rs`) partem
//! daqui; nenhum dos dois executa texto no formato do shell.

use mongodb::bson::{Bson, Document};
use serde::Deserialize;
use serde_json::Value;

use super::command::MongoCommand;

/// As operações do contrato — só leitura.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationKind {
    Find,
    Aggregate,
    CountDocuments,
    Distinct,
}

impl OperationKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Find => "find",
            Self::Aggregate => "aggregate",
            Self::CountDocuments => "countDocuments",
            Self::Distinct => "distinct",
        }
    }
}

/// Uma operação já convertida para BSON.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadOperation {
    pub collection: String,
    pub kind: OperationKind,
    pub filter: Document,
    pub projection: Option<Document>,
    pub sort: Option<Document>,
    pub limit: Option<i64>,
    pub skip: Option<i64>,
    pub pipeline: Vec<Document>,
    pub field: Option<String>,
}

/// Por que o texto não é uma operação do contrato. A mensagem é nossa (em
/// inglês, para o modelo corrigir): no máximo repete um nome que veio dele.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    Empty,
    /// JSON inválido ou fora do schema do contrato
    Invalid(String),
    /// `operation` fora das quatro de leitura (`deleteMany`, `insertOne`…)
    Operation(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => f.write_str("empty operation"),
            Self::Invalid(reason) => write!(f, "the operation is not a valid mongo-operation JSON: {reason}"),
            Self::Operation(name) => write!(
                f,
                "operation {name:?} is not allowed: use find, aggregate, countDocuments or distinct"
            ),
        }
    }
}

/// O JSON como o contrato define. `null` vale como ausente (o modelo às vezes
/// manda os opcionais assim); campo a mais é recusado.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RawOperation {
    collection: String,
    operation: String,
    #[serde(default)]
    filter: Option<Value>,
    #[serde(default)]
    projection: Option<Value>,
    #[serde(default)]
    sort: Option<Value>,
    #[serde(default)]
    limit: Option<i64>,
    #[serde(default)]
    skip: Option<i64>,
    #[serde(default)]
    pipeline: Option<Vec<Value>>,
    #[serde(default)]
    field: Option<String>,
}

pub fn parse(text: &str) -> Result<ReadOperation, ParseError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(ParseError::Empty);
    }
    let raw: RawOperation =
        serde_json::from_str(text).map_err(|e| ParseError::Invalid(e.to_string()))?;

    let kind = match raw.operation.as_str() {
        "find" => OperationKind::Find,
        "aggregate" => OperationKind::Aggregate,
        "countDocuments" => OperationKind::CountDocuments,
        "distinct" => OperationKind::Distinct,
        other => return Err(ParseError::Operation(other.to_string())),
    };
    if raw.collection.trim().is_empty() {
        return Err(ParseError::Invalid("collection is empty".into()));
    }

    // Cada campo só na operação que o usa (descrições do schema)
    let allowed: &[&str] = match kind {
        OperationKind::Find => &["filter", "projection", "sort", "limit", "skip"],
        OperationKind::Aggregate => &["pipeline"],
        OperationKind::CountDocuments => &["filter"],
        OperationKind::Distinct => &["filter", "field"],
    };
    let present = [
        ("filter", raw.filter.is_some()),
        ("projection", raw.projection.is_some()),
        ("sort", raw.sort.is_some()),
        ("limit", raw.limit.is_some()),
        ("skip", raw.skip.is_some()),
        ("pipeline", raw.pipeline.is_some()),
        ("field", raw.field.is_some()),
    ];
    if let Some((name, _)) = present.iter().find(|(name, set)| *set && !allowed.contains(name)) {
        return Err(ParseError::Invalid(format!("{name} is not used by {}", kind.name())));
    }

    if raw.limit.is_some_and(|limit| limit < 1) {
        return Err(ParseError::Invalid("limit must be at least 1".into()));
    }
    if raw.skip.is_some_and(|skip| skip < 0) {
        return Err(ParseError::Invalid("skip must not be negative".into()));
    }
    let pipeline = match (kind, raw.pipeline) {
        (OperationKind::Aggregate, None) => {
            return Err(ParseError::Invalid("aggregate requires pipeline".into()))
        }
        (_, pipeline) => pipeline
            .unwrap_or_default()
            .into_iter()
            .map(|stage| document(stage, "pipeline stage"))
            .collect::<Result<Vec<_>, _>>()?,
    };
    if kind == OperationKind::Distinct
        && raw.field.as_deref().is_none_or(|field| field.trim().is_empty())
    {
        return Err(ParseError::Invalid("distinct requires field".into()));
    }

    Ok(ReadOperation {
        collection: raw.collection,
        kind,
        filter: raw.filter.map(|value| document(value, "filter")).transpose()?.unwrap_or_default(),
        projection: raw.projection.map(|value| document(value, "projection")).transpose()?,
        sort: raw.sort.map(|value| document(value, "sort")).transpose()?,
        limit: raw.limit,
        skip: raw.skip,
        pipeline,
        field: raw.field,
    })
}

/// Objeto JSON (Extended JSON relaxado) → documento BSON.
fn document(value: Value, what: &str) -> Result<Document, ParseError> {
    if !value.is_object() {
        return Err(ParseError::Invalid(format!("{what} must be an object")));
    }
    match Bson::try_from(value) {
        Ok(Bson::Document(document)) => Ok(document),
        // Um objeto que era um valor em Extended JSON (`{"$oid": …}`) no
        // lugar de um documento
        Ok(_) => Err(ParseError::Invalid(format!("{what} must be a document, not a value"))),
        Err(e) => Err(ParseError::Invalid(format!("{what}: {e}"))),
    }
}

impl ReadOperation {
    /// O comando equivalente do editor, para conferir com o mesmo
    /// [`MongoCommand::is_write`] que separa leitura de escrita no app.
    pub fn command(&self) -> MongoCommand {
        let collection = self.collection.clone();
        match self.kind {
            OperationKind::Find => MongoCommand::Find {
                collection,
                filter: self.filter.clone(),
                projection: self.projection.clone(),
            },
            OperationKind::Aggregate => MongoCommand::Aggregate {
                collection,
                pipeline: self.pipeline.clone(),
            },
            OperationKind::CountDocuments => MongoCommand::CountDocuments {
                collection,
                filter: self.filter.clone(),
            },
            OperationKind::Distinct => MongoCommand::Distinct {
                collection,
                field: self.field.clone().unwrap_or_default(),
                filter: self.filter.clone(),
            },
        }
    }

    /// Todos os documentos da operação (filtro, projeção, ordem, estágios),
    /// para as varreduras do guard.
    pub fn documents(&self) -> Vec<&Document> {
        let mut documents = vec![&self.filter];
        documents.extend(self.projection.iter());
        documents.extend(self.sort.iter());
        documents.extend(self.pipeline.iter());
        documents
    }
}

impl ReadOperation {
    /// As coleções que a operação lê: a dela e as dos `$lookup`,
    /// `$graphLookup` e `$unionWith`, sem repetir (o preflight confere que
    /// existem).
    pub fn collections(&self) -> Vec<String> {
        let mut names = vec![self.collection.clone()];
        collect_collections(&self.pipeline, &mut names);
        names
    }
}

fn collect_collections(pipeline: &[Document], names: &mut Vec<String>) {
    let inner = |value: Option<&Bson>| -> Vec<Document> {
        match value {
            Some(Bson::Array(stages)) => stages.iter().filter_map(|stage| stage.as_document().cloned()).collect(),
            _ => Vec::new(),
        }
    };
    for stage in pipeline {
        let Some((name, body)) = stage.iter().next() else {
            continue;
        };
        let (target, nested) = match (name.as_str(), body) {
            ("$lookup" | "$graphLookup", Bson::Document(spec)) => {
                (spec.get_str("from").ok(), inner(spec.get("pipeline")))
            }
            ("$unionWith", Bson::String(coll)) => (Some(coll.as_str()), Vec::new()),
            ("$unionWith", Bson::Document(spec)) => (spec.get_str("coll").ok(), inner(spec.get("pipeline"))),
            ("$facet", Bson::Document(facets)) => {
                (None, facets.values().flat_map(|facet| inner(Some(facet))).collect())
            }
            _ => (None, Vec::new()),
        };
        if let Some(target) = target {
            if !names.iter().any(|known| known == target) {
                names.push(target.to_string());
            }
        }
        collect_collections(&nested, names);
    }
}

/// Estágios que escrevem: o executor recusa por conta própria, sem depender
/// do guard.
pub const WRITE_STAGES: [&str; 2] = ["$out", "$merge"];

/// Algum estágio do pipeline escreve? Os dois só valem no nível de cima (o
/// servidor recusa dentro de `$facet` e `$lookup`), mas a busca desce em
/// todos os documentos: na dúvida, recusa.
pub fn writes(operation: &ReadOperation) -> bool {
    fn has_write(value: &Bson) -> bool {
        match value {
            Bson::Document(document) => document
                .iter()
                .any(|(key, value)| WRITE_STAGES.contains(&key.as_str()) || has_write(value)),
            Bson::Array(items) => items.iter().any(has_write),
            _ => false,
        }
    }
    operation
        .documents()
        .into_iter()
        .any(|document| has_write(&Bson::Document(document.clone())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mongodb::bson::doc;

    #[test]
    fn parses_each_operation() {
        let find = parse(
            r#"{"collection":"orders","operation":"find","filter":{"total":{"$gt":10}},
                "projection":{"total":1},"sort":{"total":-1},"limit":5,"skip":2}"#,
        )
        .unwrap();
        assert_eq!(find.kind, OperationKind::Find);
        assert_eq!(find.filter, doc! { "total": { "$gt": 10 } });
        assert_eq!((find.limit, find.skip), (Some(5), Some(2)));
        assert!(!find.command().is_write());

        let aggregate = parse(
            r#"{"collection":"orders","operation":"aggregate","pipeline":[{"$group":{"_id":"$status","n":{"$sum":1}}}]}"#,
        )
        .unwrap();
        assert_eq!(aggregate.pipeline.len(), 1);

        let count = parse(r#"{"collection":"orders","operation":"countDocuments"}"#).unwrap();
        assert_eq!(count.filter, doc! {});

        let distinct =
            parse(r#"{"collection":"orders","operation":"distinct","field":"status","filter":null}"#).unwrap();
        assert_eq!(distinct.field.as_deref(), Some("status"));
    }

    #[test]
    fn extended_json_values_become_bson() {
        let operation = parse(
            r#"{"collection":"orders","operation":"find","filter":{
                "_id":{"$oid":"507f1f77bcf86cd799439011"},
                "createdAt":{"$gte":{"$date":"2026-01-01T00:00:00Z"}}}}"#,
        )
        .unwrap();
        assert!(matches!(operation.filter.get("_id"), Some(Bson::ObjectId(_))));
        let created = operation.filter.get_document("createdAt").unwrap();
        assert!(matches!(created.get("$gte"), Some(Bson::DateTime(_))));
    }

    #[test]
    fn rejects_what_is_out_of_contract() {
        for text in [
            "",
            "db.orders.find({})",
            r#"{"collection":"orders"}"#,
            r#"{"collection":"","operation":"find"}"#,
            r#"{"collection":"orders","operation":"find","note":"x"}"#,
            r#"{"collection":"orders","operation":"find","pipeline":[]}"#,
            r#"{"collection":"orders","operation":"aggregate"}"#,
            r#"{"collection":"orders","operation":"aggregate","pipeline":[1]}"#,
            r#"{"collection":"orders","operation":"distinct"}"#,
            r#"{"collection":"orders","operation":"find","limit":0}"#,
            r#"{"collection":"orders","operation":"find","skip":-1}"#,
            r#"{"collection":"orders","operation":"find","filter":[]}"#,
            r#"{"collection":"orders","operation":"find","filter":{"$oid":"507f1f77bcf86cd799439011"}}"#,
        ] {
            assert!(parse(text).is_err(), "{text}");
        }
        assert_eq!(
            parse(r#"{"collection":"orders","operation":"deleteMany","filter":{}}"#),
            Err(ParseError::Operation("deleteMany".into()))
        );
    }

    #[test]
    fn finds_write_stages_at_any_depth() {
        let write = |pipeline: &str| {
            writes(&parse(&format!(r#"{{"collection":"c","operation":"aggregate","pipeline":{pipeline}}}"#)).unwrap())
        };
        assert!(write(r#"[{"$match":{}},{"$out":"copy"}]"#));
        assert!(write(r#"[{"$facet":{"a":[{"$merge":{"into":"x"}}]}}]"#));
        assert!(!write(r#"[{"$match":{"note":"$out"}}]"#));
    }
}
