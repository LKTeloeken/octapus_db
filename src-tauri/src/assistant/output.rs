//! A resposta do modelo (`ModelOutput` do contrato, `output.schema.json`).
//!
//! A API já valida contra o schema antes de mandar o evento `result`; o app
//! confere de novo (falha fechado) e acrescenta o que só ele sabe: a linguagem
//! da consulta tem que ser a do banco. Fora disso é `invalid_output`.
//!
//! O app guarda uma cópia do contrato em `src-tauri/contract/`; os testes
//! daqui reprovam se ela divergir da versão esperada.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::models::QueryLanguage;

/// Versão do contrato da API que este app segue (`info.version` do openapi).
#[allow(dead_code)] // nos testes da cópia do contrato e no diagnóstico (B6)
pub const CONTRACT_VERSION: &str = "1.4.0";

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputKind {
    /// Consulta de leitura que responde à pergunta
    Query,
    /// Qualquer outro código (DDL, escrita, função): nunca executado (RN-14)
    Code,
    /// A estrutura não permite responder (C-08)
    Clarification,
}

/// A resposta de uma tentativa, já conferida.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelOutput {
    pub kind: OutputKind,
    pub language: Option<QueryLanguage>,
    pub query: Option<String>,
    /// O que a consulta retorna, no idioma da pergunta
    pub explanation: String,
    /// Premissas assumidas (RN-20)
    pub assumptions: Vec<String>,
    pub clarification_question: Option<String>,
}

/// Os campos do contrato: todos obrigatórios, os opcionais podem ser `null`.
const FIELDS: [&str; 6] = [
    "kind",
    "language",
    "query",
    "explanation",
    "assumptions",
    "clarificationQuestion",
];

/// Confere a resposta (o `data` do evento `result`) e a linguagem esperada
/// para o banco. `Err` traz o motivo, para o registro da tentativa — nunca o
/// texto da consulta.
pub fn parse(value: Value, expected: QueryLanguage) -> Result<ModelOutput, String> {
    let Value::Object(fields) = &value else {
        return Err("output is not a JSON object".into());
    };
    for field in FIELDS {
        if !fields.contains_key(field) {
            return Err(format!("missing field {field}"));
        }
    }
    let output: ModelOutput =
        serde_json::from_value(value).map_err(|e| format!("output out of contract: {e}"))?;

    let blank = |text: &Option<String>| text.as_deref().is_none_or(|text| text.trim().is_empty());
    match output.kind {
        OutputKind::Clarification => {
            if blank(&output.clarification_question) {
                return Err("clarification without clarificationQuestion".into());
            }
            if output.query.is_some() || output.language.is_some() {
                return Err("clarification with query or language".into());
            }
        }
        OutputKind::Query | OutputKind::Code => {
            if output.language.is_none() || blank(&output.query) {
                return Err(format!("{:?} without language or query", output.kind).to_lowercase());
            }
            if output.clarification_question.is_some() {
                return Err(format!("{:?} with clarificationQuestion", output.kind).to_lowercase());
            }
            if output.language != Some(expected) {
                return Err(format!(
                    "language {:?} does not match the database ({expected:?})",
                    output.language.expect("conferida acima")
                ));
            }
        }
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use sha2::{Digest, Sha256};

    fn query(sql: &str) -> Value {
        json!({
            "kind": "query",
            "language": "sql",
            "query": sql,
            "explanation": "Total por cliente",
            "assumptions": ["total inclui impostos"],
            "clarificationQuestion": null,
        })
    }

    #[test]
    fn accepts_each_kind() {
        let output = parse(query("SELECT 1"), QueryLanguage::Sql).unwrap();
        assert_eq!(output.kind, OutputKind::Query);
        assert_eq!(output.assumptions, ["total inclui impostos"]);

        let code = json!({
            "kind": "code", "language": "sql", "query": "DELETE FROM t",
            "explanation": "Apaga tudo", "assumptions": [], "clarificationQuestion": null,
        });
        assert_eq!(parse(code, QueryLanguage::Sql).unwrap().kind, OutputKind::Code);

        let ask = json!({
            "kind": "clarification", "language": null, "query": null,
            "explanation": "", "assumptions": [], "clarificationQuestion": "Qual tabela?",
        });
        assert_eq!(parse(ask, QueryLanguage::Sql).unwrap().kind, OutputKind::Clarification);
    }

    #[test]
    fn rejects_what_is_out_of_contract() {
        let mut missing = query("SELECT 1");
        missing.as_object_mut().unwrap().remove("assumptions");
        let mut extra = query("SELECT 1");
        extra["note"] = json!("x");
        let mut bad_kind = query("SELECT 1");
        bad_kind["kind"] = json!("answer");
        let mut empty_query = query("  ");
        empty_query["kind"] = json!("query");
        let mut asks_too = query("SELECT 1");
        asks_too["clarificationQuestion"] = json!("?");
        let clarification_with_query = json!({
            "kind": "clarification", "language": "sql", "query": "SELECT 1",
            "explanation": "", "assumptions": [], "clarificationQuestion": "Qual?",
        });
        let clarification_without_question = json!({
            "kind": "clarification", "language": null, "query": null,
            "explanation": "", "assumptions": [], "clarificationQuestion": " ",
        });

        for value in [
            json!("SELECT 1"),
            missing,
            extra,
            bad_kind,
            empty_query,
            asks_too,
            clarification_with_query,
            clarification_without_question,
        ] {
            assert!(parse(value.clone(), QueryLanguage::Sql).is_err(), "{value}");
        }
    }

    #[test]
    fn language_must_match_the_database() {
        let error = parse(query("SELECT 1"), QueryLanguage::Mongo).unwrap_err();
        assert!(error.contains("does not match"), "{error}");
    }

    // ── Cópia do contrato ───────────────────────────────────────────────────

    /// SHA-256 dos arquivos de `src-tauri/contract/` no contrato
    /// [`CONTRACT_VERSION`]. Mudou um deles? Atualize a cópia junto com a API
    /// (e este código), não só o arquivo.
    const CONTRACT_FILES: [(&str, &str, &str); 3] = [
        (
            "output.schema.json",
            include_str!("../../contract/output.schema.json"),
            "401f3ddd6c7c488adda92280c58853d848f3dc26193db0238d114ccb52a38f66",
        ),
        (
            "mongo-operation.schema.json",
            include_str!("../../contract/mongo-operation.schema.json"),
            "7719ce6b66d77239791c40f88a9c0a546d5f9204179c5cfab47b1bfac885cd6e",
        ),
        (
            "redis-plan.schema.json",
            include_str!("../../contract/redis-plan.schema.json"),
            "782dfb7d89d997bdf74a210075cb43853dd3423e4ad29c9234647905c3490d2a",
        ),
    ];

    #[test]
    fn contract_copy_matches_the_expected_version() {
        for (name, text, expected) in CONTRACT_FILES {
            let sha = hex::encode(Sha256::digest(text.as_bytes()));
            assert_eq!(sha, expected, "{name} divergiu do contrato {CONTRACT_VERSION}");
        }
    }

    /// Com a pasta do contrato da API (`OCTAPUS_CONTRACT_DIR`), confere byte a
    /// byte — o CI dos dois lados. Sem a variável, não faz nada.
    #[test]
    fn contract_copy_matches_the_api_when_available() {
        let Ok(dir) = std::env::var("OCTAPUS_CONTRACT_DIR") else {
            return;
        };
        for (name, text, _) in CONTRACT_FILES {
            let upstream = std::fs::read_to_string(std::path::Path::new(&dir).join(name))
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(text, upstream, "{name} diverge da API");
        }
    }

    #[test]
    fn rust_types_follow_the_output_schema() {
        let schema: Value = serde_json::from_str(CONTRACT_FILES[0].1).unwrap();
        let required: Vec<&str> = schema["required"]
            .as_array()
            .unwrap()
            .iter()
            .map(|field| field.as_str().unwrap())
            .collect();
        assert_eq!(required, FIELDS);

        let kinds = &schema["properties"]["kind"]["enum"];
        assert_eq!(*kinds, json!(["query", "code", "clarification"]));
        let languages = &schema["properties"]["language"]["anyOf"][0]["enum"];
        for language in [QueryLanguage::Sql, QueryLanguage::Mongo, QueryLanguage::RedisPlan] {
            let name = serde_json::to_value(language).unwrap();
            assert!(languages.as_array().unwrap().contains(&name), "{name}");
        }

        // Os nomes que o serde gera são os do schema
        let output = parse(query("SELECT 1"), QueryLanguage::Sql).unwrap();
        let serialized = serde_json::to_value(&output).unwrap();
        let mut keys: Vec<&str> = serialized.as_object().unwrap().keys().map(String::as_str).collect();
        keys.sort_unstable();
        let mut expected = FIELDS;
        expected.sort_unstable();
        assert_eq!(keys, expected);
    }
}
