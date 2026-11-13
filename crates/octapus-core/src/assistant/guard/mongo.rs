//! Guard das operações Mongo geradas pelo assistente.
//!
//! Falha fechado: na dúvida, recusa. A operação precisa parsear pelo contrato
//! (`adapters/mongo/operation.rs`: só `find`, `aggregate`, `countDocuments` e
//! `distinct`) e ser leitura pelo mesmo `MongoCommand::is_write` do editor.
//! Depois, a varredura dos documentos, em qualquer profundidade:
//!
//! - **estágios do pipeline por allowlist**, inclusive nos sub-pipelines
//!   (`$lookup`, `$unionWith`, `$facet`): estágio desconhecido não passa;
//!   `$out` e `$merge` escrevem, e os de administração (`$currentOp`,
//!   `$collStats`, `$changeStream`…) não são consulta de dados;
//! - **JavaScript no servidor:** `$where`, `$function`, `$accumulator` e
//!   qualquer valor BSON de código (`{"$code": …}`);
//! - **coleções:** a da operação e as que `$lookup`, `$graphLookup` e
//!   `$unionWith` leem, sem `$`, sem as `system.*` e sem outro database.
//!
//! Não é a única barreira: o executor (`adapters/mongo/read_only.rs`) monta o
//! comando pelo tipo da operação, recusa `$out`/`$merge` por conta própria e
//! roda com `maxTimeMS` e teto de documentos.

use std::fmt;

use mongodb::bson::{Bson, Document};

use crate::adapters::mongo::operation::{self, ParseError, ReadOperation, WRITE_STAGES};

/// Por que o guard recusou a operação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rejection {
    Empty,
    /// Não é JSON do contrato: na dúvida, não executa
    Unparseable(String),
    /// `operation` que não é de leitura (`deleteMany`, `insertOne`…)
    NotAQuery(String),
    /// `$out` ou `$merge`
    WriteStage(String),
    /// Estágio fora da allowlist (administração, desconhecido)
    ForbiddenStage(String),
    /// `$where`, `$function`, `$accumulator` ou código BSON
    JavaScript(String),
    /// Coleção `system.*`, com `$`, ou de outro database
    ForbiddenCollection,
    /// Estágio que não é um documento com uma chave só (`{"$match": …}`)
    MalformedStage,
}

impl Rejection {
    /// Código estável, para log e classificação (nunca o texto da operação).
    pub fn code(&self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::Unparseable(_) => "unparseable",
            Self::NotAQuery(_) => "not_a_query",
            Self::WriteStage(_) => "write_stage",
            Self::ForbiddenStage(_) => "forbidden_stage",
            Self::JavaScript(_) => "javascript",
            Self::ForbiddenCollection => "forbidden_collection",
            Self::MalformedStage => "malformed_stage",
        }
    }
}

impl fmt::Display for Rejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("empty operation"),
            Self::Unparseable(reason) => f.write_str(reason),
            Self::NotAQuery(reason) => f.write_str(reason),
            Self::WriteStage(stage) => write!(f, "{stage} writes to the database and is not allowed"),
            Self::ForbiddenStage(stage) => write!(f, "pipeline stage {stage} is not allowed"),
            Self::JavaScript(operator) => {
                write!(f, "{operator} runs JavaScript on the server and is not allowed")
            }
            Self::ForbiddenCollection => {
                f.write_str("only collections of the current database can be read (no system collections)")
            }
            Self::MalformedStage => f.write_str("each pipeline stage must be an object with exactly one stage"),
        }
    }
}

/// Estágios de leitura da documentação do MongoDB (Aggregation Stages). O
/// que não está aqui é recusado — inclusive estágio novo de uma versão futura.
const ALLOWED_STAGES: &[&str] = &[
    "$addFields",
    "$bucket",
    "$bucketAuto",
    "$count",
    "$densify",
    "$facet",
    "$fill",
    "$geoNear",
    "$graphLookup",
    "$group",
    "$limit",
    "$lookup",
    "$match",
    "$project",
    "$redact",
    "$replaceRoot",
    "$replaceWith",
    "$sample",
    "$search",
    "$searchMeta",
    "$set",
    "$setWindowFields",
    "$skip",
    "$sort",
    "$sortByCount",
    "$unionWith",
    "$unset",
    "$unwind",
    "$vectorSearch",
];

/// Operadores que executam JavaScript no servidor.
const JAVASCRIPT_OPERATORS: [&str; 3] = ["$where", "$function", "$accumulator"];

/// Confere a operação (o texto do `query` da resposta do modelo) e devolve a
/// operação lida.
pub fn check(text: &str) -> Result<ReadOperation, Rejection> {
    let operation = operation::parse(text).map_err(|error| match error {
        ParseError::Empty => Rejection::Empty,
        ParseError::Operation(_) => Rejection::NotAQuery(error.to_string()),
        ParseError::Invalid(_) => Rejection::Unparseable(error.to_string()),
    })?;
    if operation.command().is_write() {
        return Err(Rejection::NotAQuery(format!(
            "operation {} is not a read",
            operation.kind.name()
        )));
    }

    check_collection_name(&operation.collection)?;
    if let Some(field) = operation.field.as_deref() {
        if field.starts_with('$') || field.contains('\0') {
            return Err(Rejection::Unparseable("distinct field must be a field path without $".into()));
        }
    }
    check_pipeline(&operation.pipeline)?;
    for document in operation.documents() {
        scan(&Bson::Document(document.clone()))?;
    }
    Ok(operation)
}

fn check_collection_name(name: &str) -> Result<(), Rejection> {
    let name = name.trim();
    if name.is_empty() || name.contains('$') || name.contains('\0') || name.starts_with("system.") {
        return Err(Rejection::ForbiddenCollection);
    }
    Ok(())
}

/// Cada estágio: um documento com uma chave só, da allowlist; os
/// sub-pipelines passam pela mesma regra.
fn check_pipeline(pipeline: &[Document]) -> Result<(), Rejection> {
    for stage in pipeline {
        if stage.len() != 1 {
            return Err(Rejection::MalformedStage);
        }
        let (name, body) = stage.iter().next().expect("um estágio com uma chave");
        if WRITE_STAGES.contains(&name.as_str()) {
            return Err(Rejection::WriteStage(name.clone()));
        }
        if !ALLOWED_STAGES.contains(&name.as_str()) {
            return Err(Rejection::ForbiddenStage(stage_label(name)));
        }
        match (name.as_str(), body) {
            ("$lookup" | "$graphLookup", Bson::Document(spec)) => {
                // `from: {db, coll}` leria outro database
                match spec.get("from") {
                    Some(Bson::String(from)) => check_collection_name(from)?,
                    None if name == "$lookup" => {} // `$lookup` só com `pipeline` (`$documents`)
                    _ => return Err(Rejection::ForbiddenCollection),
                }
                check_inner(spec.get("pipeline"))?;
            }
            ("$unionWith", Bson::String(coll)) => check_collection_name(coll)?,
            ("$unionWith", Bson::Document(spec)) => {
                match spec.get("coll") {
                    Some(Bson::String(coll)) => check_collection_name(coll)?,
                    _ => return Err(Rejection::ForbiddenCollection),
                }
                check_inner(spec.get("pipeline"))?;
            }
            ("$unionWith", _) => return Err(Rejection::ForbiddenCollection),
            ("$facet", Bson::Document(facets)) => {
                for (_, inner) in facets {
                    check_inner(Some(inner))?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn check_inner(pipeline: Option<&Bson>) -> Result<(), Rejection> {
    match pipeline {
        None => Ok(()),
        Some(Bson::Array(stages)) => {
            let stages = stages
                .iter()
                .map(|stage| stage.as_document().cloned().ok_or(Rejection::MalformedStage))
                .collect::<Result<Vec<_>, _>>()?;
            check_pipeline(&stages)
        }
        Some(_) => Err(Rejection::MalformedStage),
    }
}

/// O nome do estágio na mensagem: os que começam com `$` são da linguagem;
/// outra chave vem como "unknown" (pode ser um nome qualquer).
fn stage_label(name: &str) -> String {
    let looks_like_stage = name.starts_with('$') && name.len() <= 40 && name[1..].chars().all(|c| c.is_ascii_alphanumeric());
    if looks_like_stage {
        name.to_string()
    } else {
        "(unknown)".to_string()
    }
}

/// Varredura em qualquer profundidade: JavaScript e estágios de escrita
/// escondidos em qualquer lugar.
fn scan(value: &Bson) -> Result<(), Rejection> {
    match value {
        Bson::Document(document) => {
            for (key, value) in document {
                if JAVASCRIPT_OPERATORS.contains(&key.as_str()) {
                    return Err(Rejection::JavaScript(key.clone()));
                }
                if WRITE_STAGES.contains(&key.as_str()) {
                    return Err(Rejection::WriteStage(key.clone()));
                }
                scan(value)?;
            }
            Ok(())
        }
        Bson::Array(items) => items.iter().try_for_each(scan),
        Bson::JavaScriptCode(_) | Bson::JavaScriptCodeWithScope(_) => {
            Err(Rejection::JavaScript("JavaScript code".into()))
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find(filter: &str) -> String {
        format!(r#"{{"collection":"orders","operation":"find","filter":{filter}}}"#)
    }

    fn aggregate(pipeline: &str) -> String {
        format!(r#"{{"collection":"orders","operation":"aggregate","pipeline":{pipeline}}}"#)
    }

    fn code(text: &str) -> &'static str {
        check(text).unwrap_err().code()
    }

    #[test]
    fn accepts_read_operations() {
        for text in [
            find(r#"{"status":"paid","total":{"$gte":10}}"#),
            find(r#"{"$expr":{"$gt":["$total","$discount"]}}"#),
            aggregate(r#"[{"$match":{"status":"paid"}},{"$group":{"_id":"$customer","n":{"$sum":1}}},{"$sort":{"n":-1}},{"$limit":10}]"#),
            aggregate(r#"[{"$lookup":{"from":"customers","localField":"customerId","foreignField":"_id","as":"c"}},{"$unwind":"$c"}]"#),
            aggregate(r#"[{"$facet":{"porStatus":[{"$sortByCount":"$status"}],"total":[{"$count":"n"}]}}]"#),
            aggregate(r#"[{"$unionWith":{"coll":"archived_orders","pipeline":[{"$match":{}}]}}]"#),
            // `$out` como valor (texto), não como estágio
            find(r#"{"note":"$out"}"#),
            r#"{"collection":"orders","operation":"countDocuments","filter":{}}"#.to_string(),
            r#"{"collection":"orders","operation":"distinct","field":"status"}"#.to_string(),
        ] {
            assert!(check(&text).is_ok(), "{text}: {:?}", check(&text));
        }
    }

    #[test]
    fn rejects_writes_and_javascript_at_any_depth() {
        let cases = [
            (r#"{"collection":"orders","operation":"deleteMany","filter":{}}"#.to_string(), "not_a_query"),
            (r#"{"collection":"orders","operation":"insertOne"}"#.to_string(), "not_a_query"),
            (aggregate(r#"[{"$match":{}},{"$out":"copy"}]"#), "write_stage"),
            (aggregate(r#"[{"$merge":{"into":"copy"}}]"#), "write_stage"),
            (aggregate(r#"[{"$facet":{"a":[{"$out":"copy"}]}}]"#), "write_stage"),
            (aggregate(r#"[{"$lookup":{"from":"c","as":"x","pipeline":[{"$merge":"y"}]}}]"#), "write_stage"),
            (find(r#"{"$where":"this.total > 1"}"#), "javascript"),
            (find(r#"{"$expr":{"$function":{"body":"function(){return true}","args":[],"lang":"js"}}}"#), "javascript"),
            (aggregate(r#"[{"$group":{"_id":null,"x":{"$accumulator":{"init":"function(){}"}}}}]"#), "javascript"),
            (find(r#"{"a":{"$code":"function(){}"}}"#), "javascript"),
            (find(r#"{"a":{"$in":[{"b":{"$where":"1"}}]}}"#), "javascript"),
            (aggregate(r#"[{"$currentOp":{}}]"#), "forbidden_stage"),
            (aggregate(r#"[{"$collStats":{"storageStats":{}}}]"#), "forbidden_stage"),
            (aggregate(r#"[{"$changeStream":{}}]"#), "forbidden_stage"),
            (aggregate(r#"[{"$listSessions":{}}]"#), "forbidden_stage"),
            (aggregate(r#"[{"$futureStage":{}}]"#), "forbidden_stage"),
            (aggregate(r#"[{"$match":{},"$limit":1}]"#), "malformed_stage"),
            (aggregate(r#"[{}]"#), "malformed_stage"),
            (aggregate(r#"[{"$lookup":{"from":{"db":"admin","coll":"system.users"},"as":"x"}}]"#), "forbidden_collection"),
            (aggregate(r#"[{"$unionWith":"system.profile"}]"#), "forbidden_collection"),
            (r#"{"collection":"system.users","operation":"find"}"#.to_string(), "forbidden_collection"),
            (r#"{"collection":"a$b","operation":"find"}"#.to_string(), "forbidden_collection"),
            (r#"{"collection":"orders","operation":"distinct","field":"$status"}"#.to_string(), "unparseable"),
            ("db.orders.find({})".to_string(), "unparseable"),
            (find(r#"{"a":1},"pipeline":[{"$out":"x"}]"#), "unparseable"),
            ("   ".to_string(), "empty"),
        ];
        for (text, expected) in cases {
            assert_eq!(code(&text), expected, "{text}");
        }
    }

    #[test]
    fn messages_never_echo_values() {
        // O valor do filtro não vai para a mensagem (ela volta ao modelo)
        let rejection = check(&find(r#"{"$where":"CANARY_7f3a"}"#)).unwrap_err();
        assert!(!rejection.to_string().contains("CANARY"), "{rejection}");
        let rejection = check(&aggregate(r#"[{"CANARY_9b1c":{}}]"#)).unwrap_err();
        assert_eq!(rejection.to_string(), "pipeline stage (unknown) is not allowed");
    }

    #[test]
    fn lists_the_collections_it_reads() {
        let operation = check(&aggregate(
            r#"[{"$lookup":{"from":"customers","as":"c","localField":"a","foreignField":"b"}},
                {"$unionWith":{"coll":"archived","pipeline":[{"$lookup":{"from":"customers","as":"d","pipeline":[]}}]}}]"#,
        ))
        .unwrap();
        assert_eq!(operation.collections(), ["orders", "customers", "archived"]);
    }
}
