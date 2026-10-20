//! As mensagens de uma tentativa (`AttemptRequest` do contrato).
//!
//! 1. `system`: o prompt do paradigma, com os bytes exatos registrados (a API
//!    confere o SHA-256) — por isso nada variável vai nele;
//! 2. os turnos anteriores da conversa (C-12), do mais antigo ao mais novo:
//!    a pergunta e a resposta do modelo, só os que cabem no orçamento,
//!    preferindo os mais recentes;
//! 3. o turno atual: a estrutura (`<schema>`) e a pergunta;
//! 4. em correção, cada tentativa que falhou neste turno e o erro sanitizado
//!    dela (`<previous_attempt_failed>`), na ordem — a última mensagem é
//!    sempre do usuário, como o contrato exige.
//!
//! Mudar o formato daqui muda o que o modelo vê: trate como uma versão nova
//! do prompt.

// Consumido pelo orquestrador do turno (B6)
#![allow(dead_code)]

use serde::Serialize;

use super::failure::FailureClass;
use super::output::ModelOutput;
use super::prompts::Prompt;
use super::schema_context::{budget_tokens, estimate_tokens, SchemaContext};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

/// O corpo de `POST /v1/turns/{turnId}/attempts`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptRequest {
    pub prompt_version: String,
    pub messages: Vec<Message>,
    pub context: AttemptContext,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttemptContext {
    /// A estrutura enviada é um recorte (RN-26)
    pub schema_truncated: bool,
    pub schema_tokens_estimate: usize,
    /// Classe da falha da tentativa anterior, quando esta é uma correção (C-11)
    pub previous_failure: Option<FailureClass>,
}

/// Um turno anterior da conversa: a pergunta e a resposta que o modelo deu
/// (a da última tentativa gerada).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryTurn {
    pub question: String,
    pub output: ModelOutput,
}

/// Onde a tentativa falhou.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailedStage {
    /// O guard de leitura recusou antes de ir ao banco
    Guard,
    /// O `EXPLAIN` (sem executar)
    Preflight,
    Execution,
}

impl FailedStage {
    fn as_str(self) -> &'static str {
        match self {
            Self::Guard => "guard",
            Self::Preflight => "preflight",
            Self::Execution => "execution",
        }
    }
}

/// Uma tentativa deste turno que falhou: a resposta do modelo e o porquê. O
/// erro já vem sanitizado (`failure::sanitize`, RN-10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Correction {
    pub output: ModelOutput,
    pub stage: FailedStage,
    pub class: FailureClass,
    pub error_sanitized: Option<String>,
}

/// O histórico ganha metade do orçamento da estrutura (pelo menos 500 tokens):
/// é contexto da conversa, não a resposta.
pub fn history_budget_tokens(context_window: i64) -> usize {
    (budget_tokens(context_window) / 2).max(500)
}

/// O pedido completo da tentativa.
pub fn attempt_request(
    prompt: Prompt,
    schema: &SchemaContext,
    history: &[HistoryTurn],
    question: &str,
    corrections: &[Correction],
    context_window: i64,
) -> AttemptRequest {
    let mut messages = vec![Message {
        role: Role::System,
        content: prompt.text.to_string(),
    }];
    messages.extend(history_messages(history, history_budget_tokens(context_window)));
    messages.push(Message {
        role: Role::User,
        content: current_turn(schema, question),
    });
    for correction in corrections {
        messages.push(Message {
            role: Role::Assistant,
            content: output_json(&correction.output),
        });
        messages.push(Message {
            role: Role::User,
            content: correction_text(correction),
        });
    }

    AttemptRequest {
        prompt_version: prompt.version.to_string(),
        messages,
        context: AttemptContext {
            schema_truncated: schema.truncated,
            schema_tokens_estimate: schema.tokens_estimate,
            previous_failure: corrections.last().map(|correction| correction.class),
        },
    }
}

/// Os turnos que cabem no orçamento, dos mais recentes para trás, devolvidos
/// em ordem cronológica. Um turno entra inteiro ou não entra.
fn history_messages(history: &[HistoryTurn], budget: usize) -> Vec<Message> {
    let mut kept: Vec<[Message; 2]> = Vec::new();
    let mut used = 0;
    for turn in history.iter().rev() {
        let pair = [
            Message {
                role: Role::User,
                content: question_block(&turn.question),
            },
            Message {
                role: Role::Assistant,
                content: output_json(&turn.output),
            },
        ];
        let cost = estimate_tokens(&pair[0].content) + estimate_tokens(&pair[1].content);
        if used + cost > budget {
            break;
        }
        used += cost;
        kept.push(pair);
    }
    kept.into_iter().rev().flatten().collect()
}

fn current_turn(schema: &SchemaContext, question: &str) -> String {
    let open = if schema.truncated {
        "<schema truncated=\"true\">"
    } else {
        "<schema>"
    };
    format!("{open}\n{}\n</schema>\n{}", schema.ddl, question_block(question))
}

fn question_block(question: &str) -> String {
    format!("<question>\n{}\n</question>", question.trim())
}

/// A resposta do modelo como ele a deu: o JSON do contrato, compacto.
fn output_json(output: &ModelOutput) -> String {
    serde_json::to_string(output).expect("ModelOutput sempre serializa")
}

fn correction_text(correction: &Correction) -> String {
    let class = serde_json::to_value(correction.class)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default();
    let error = correction
        .error_sanitized
        .as_deref()
        .map(str::trim)
        .filter(|error| !error.is_empty())
        .unwrap_or("(no error message)");
    format!(
        "<previous_attempt_failed stage=\"{}\" class=\"{class}\">\n{error}\n</previous_attempt_failed>\n\
         Fix the query and answer again in the same JSON format.",
        correction.stage.as_str()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::output::OutputKind;
    use crate::assistant::prompts;
    use crate::models::{DatabaseType, QueryLanguage};

    fn output(sql: &str) -> ModelOutput {
        ModelOutput {
            kind: OutputKind::Query,
            language: Some(QueryLanguage::Sql),
            query: Some(sql.into()),
            explanation: "Lista".into(),
            assumptions: vec![],
            clarification_question: None,
        }
    }

    fn schema(truncated: bool) -> SchemaContext {
        let ddl = "-- PostgreSQL. Schemas: \"public\"\nCREATE TABLE \"public\".\"orders\" (\n  \"id\" integer\n);".to_string();
        SchemaContext {
            tokens_estimate: estimate_tokens(&ddl),
            ddl,
            tables: 1,
            truncated,
        }
    }

    fn turn(question: &str) -> HistoryTurn {
        HistoryTurn {
            question: question.into(),
            output: output("SELECT 1"),
        }
    }

    #[test]
    fn first_attempt_has_system_and_the_current_turn() {
        let prompt = prompts::for_db(DatabaseType::Postgres).unwrap();
        let request = attempt_request(prompt, &schema(false), &[], "  quantos pedidos?  ", &[], 200_000);

        assert_eq!(request.prompt_version, "postgres-v1");
        assert_eq!(request.messages.len(), 2);
        // A system é o arquivo inteiro, byte a byte (a API confere o hash)
        assert_eq!(request.messages[0].role, Role::System);
        assert_eq!(request.messages[0].content, prompt.text);
        assert_eq!(
            request.messages[1].content,
            "<schema>\n-- PostgreSQL. Schemas: \"public\"\nCREATE TABLE \"public\".\"orders\" (\n  \"id\" integer\n);\n</schema>\n<question>\nquantos pedidos?\n</question>"
        );
        assert_eq!(
            request.context,
            AttemptContext {
                schema_truncated: false,
                schema_tokens_estimate: schema(false).tokens_estimate,
                previous_failure: None,
            }
        );
    }

    #[test]
    fn history_alternates_and_keeps_the_most_recent_within_budget() {
        let prompt = prompts::for_db(DatabaseType::Sqlite).unwrap();
        let history = [turn("primeira"), turn("segunda"), turn("terceira")];
        let request = attempt_request(prompt, &schema(true), &history, "quarta", &[], 200_000);

        let roles: Vec<Role> = request.messages.iter().map(|m| m.role).collect();
        assert_eq!(
            roles,
            [Role::System, Role::User, Role::Assistant, Role::User, Role::Assistant, Role::User, Role::Assistant, Role::User]
        );
        assert_eq!(request.messages[1].content, "<question>\nprimeira\n</question>");
        assert_eq!(
            request.messages[2].content,
            r#"{"kind":"query","language":"sql","query":"SELECT 1","explanation":"Lista","assumptions":[],"clarificationQuestion":null}"#
        );
        assert!(request.messages[7].content.starts_with("<schema truncated=\"true\">\n"));
        assert!(request.context.schema_truncated);

        // Orçamento para um turno só: fica o mais recente
        let kept = history_messages(&history, 50);
        assert_eq!(kept.len(), 2);
        assert_eq!(kept[0].content, "<question>\nterceira\n</question>");
        assert!(history_messages(&history, 10).is_empty());
    }

    #[test]
    fn corrections_follow_the_turn_and_end_with_the_user() {
        let prompt = prompts::for_db(DatabaseType::Postgres).unwrap();
        let corrections = [
            Correction {
                output: output("SELECT nome FROM orders"),
                stage: FailedStage::Preflight,
                class: FailureClass::UnknownObject,
                error_sanitized: Some("42703: column \"nome\" does not exist".into()),
            },
            Correction {
                output: output("SELECT name::int FROM orders"),
                stage: FailedStage::Execution,
                class: FailureClass::TypeMismatch,
                error_sanitized: None,
            },
        ];
        let request = attempt_request(prompt, &schema(false), &[], "nomes", &corrections, 200_000);

        let roles: Vec<Role> = request.messages.iter().map(|m| m.role).collect();
        assert_eq!(roles, [Role::System, Role::User, Role::Assistant, Role::User, Role::Assistant, Role::User]);
        assert!(request.messages[2].content.contains("SELECT nome FROM orders"));
        assert_eq!(
            request.messages[3].content,
            "<previous_attempt_failed stage=\"preflight\" class=\"unknown_object\">\n\
             42703: column \"nome\" does not exist\n</previous_attempt_failed>\n\
             Fix the query and answer again in the same JSON format."
        );
        assert!(request.messages[5].content.contains("class=\"type_mismatch\">\n(no error message)\n"));
        assert_eq!(request.context.previous_failure, Some(FailureClass::TypeMismatch));
    }

    #[test]
    fn request_serializes_as_the_contract() {
        let prompt = prompts::for_db(DatabaseType::Postgres).unwrap();
        let request = attempt_request(prompt, &schema(false), &[], "x", &[], 8_000);
        let value = serde_json::to_value(&request).unwrap();
        assert_eq!(value["promptVersion"], "postgres-v1");
        assert_eq!(value["messages"][0]["role"], "system");
        assert_eq!(
            value["context"],
            serde_json::json!({
                "schemaTruncated": false,
                "schemaTokensEstimate": schema(false).tokens_estimate,
                "previousFailure": null,
            })
        );
        assert_eq!(history_budget_tokens(8_000), 500);
        assert_eq!(history_budget_tokens(200_000), 8_000);
    }
}
