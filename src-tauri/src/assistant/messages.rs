//! As mensagens de uma tentativa (`AttemptRequest` do contrato).
//!
//! 1. `system`: o prompt do paradigma, com os bytes exatos registrados (a API
//!    confere o SHA-256) — por isso nada variável vai nele;
//! 2. os turnos anteriores da conversa (C-12), do mais antigo ao mais novo:
//!    a pergunta e a resposta do modelo, só os que cabem no orçamento,
//!    preferindo os mais recentes;
//! 3. o turno atual: a estrutura (`<schema>`) e a pergunta;
//! 4. as rodadas deste turno, na ordem ([`Round`]): cada tentativa que falhou
//!    e o erro sanitizado dela (`<previous_attempt_failed>`), e cada pedido de
//!    estrutura do modelo com o que o app achou (`<requested_structure>`) — a
//!    última mensagem é sempre do usuário, como o contrato exige.
//!
//! Mudar o formato daqui muda o que o modelo vê: trate como uma versão nova
//! do prompt.

use serde::Serialize;

use super::failure::FailureClass;
use super::output::ModelOutput;
use super::prompts::Prompt;
use super::requested::RequestedStructure;
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
/// (a da última tentativa gerada que não foi pedido de estrutura — numa
/// resposta em texto, o próprio texto).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryTurn {
    pub question: String,
    pub output: ModelOutput,
}

/// Onde a tentativa falhou (o mesmo `AssistantStage` dos eventos do turno).
pub use crate::models::AssistantStage as FailedStage;

fn stage_name(stage: FailedStage) -> &'static str {
    match stage {
        FailedStage::Guard => "guard",
        FailedStage::Preflight => "preflight",
        FailedStage::Execution => "execution",
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

/// Um pedido de estrutura do modelo neste turno: o pedido como ele deu e o
/// que o app respondeu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructureRound {
    pub output: ModelOutput,
    pub reply: StructureReply,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructureReply {
    /// A estrutura achada e os nomes não encontrados
    Found(RequestedStructure),
    /// O teto de pedidos do turno já tinha sido atingido: nada foi buscado
    LimitReached,
}

/// O que aconteceu depois da pergunta, neste turno, na ordem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Round {
    Correction(Correction),
    Structure(StructureRound),
}

impl Round {
    fn output(&self) -> &ModelOutput {
        match self {
            Round::Correction(correction) => &correction.output,
            Round::Structure(round) => &round.output,
        }
    }
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
    rounds: &[Round],
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
    let mut truncated = schema.truncated;
    let mut tokens = schema.tokens_estimate;
    for round in rounds {
        messages.push(Message {
            role: Role::Assistant,
            content: output_json(round.output()),
        });
        let content = match round {
            Round::Correction(correction) => correction_text(correction),
            Round::Structure(round) => {
                if let StructureReply::Found(found) = &round.reply {
                    truncated |= found.truncated;
                    tokens += found.tokens_estimate;
                }
                structure_text(&round.reply)
            }
        };
        messages.push(Message {
            role: Role::User,
            content,
        });
    }

    AttemptRequest {
        prompt_version: prompt.version.to_string(),
        messages,
        context: AttemptContext {
            schema_truncated: truncated,
            schema_tokens_estimate: tokens,
            // A falha da tentativa logo antes; depois de um pedido de
            // estrutura, não há
            previous_failure: match rounds.last() {
                Some(Round::Correction(correction)) => Some(correction.class),
                _ => None,
            },
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

/// A resposta a um pedido de estrutura. A estrutura é dado do banco (RN-22),
/// com a mesma convenção do `<schema>`.
fn structure_text(reply: &StructureReply) -> String {
    match reply {
        StructureReply::Found(found) => {
            let open = if found.truncated {
                "<requested_structure truncated=\"true\">"
            } else {
                "<requested_structure>"
            };
            format!(
                "{open}\n{}\n</requested_structure>\n\
                 Answer the question now in the same JSON format. Do not request names that were not found again.",
                found.text
            )
        }
        StructureReply::LimitReached => "<requested_structure limit_reached=\"true\">\n\
             -- No more structure can be requested for this question.\n</requested_structure>\n\
             Answer the question now with the structure you have, in the same JSON format."
            .to_string(),
    }
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
        stage_name(correction.stage)
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
            structure_request: None,
        }
    }

    fn schema(truncated: bool) -> SchemaContext {
        let ddl = "-- PostgreSQL. Schemas: \"public\"\nCREATE TABLE \"public\".\"orders\" (\n  \"id\" integer\n);".to_string();
        SchemaContext {
            tokens_estimate: estimate_tokens(&ddl),
            ddl,
            tables: 1,
            truncated,
            mentions: Default::default(),
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

        assert_eq!(request.prompt_version, "postgres-v2");
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
            r#"{"kind":"query","language":"sql","query":"SELECT 1","explanation":"Lista","assumptions":[],"clarificationQuestion":null,"structureRequest":null}"#
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
            Round::Correction(Correction {
                output: output("SELECT nome FROM orders"),
                stage: FailedStage::Preflight,
                class: FailureClass::UnknownObject,
                error_sanitized: Some("42703: column \"nome\" does not exist".into()),
            }),
            Round::Correction(Correction {
                output: output("SELECT name::int FROM orders"),
                stage: FailedStage::Execution,
                class: FailureClass::TypeMismatch,
                error_sanitized: None,
            }),
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

    fn structure_request(names: &[&str]) -> ModelOutput {
        ModelOutput {
            kind: OutputKind::StructureRequest,
            language: None,
            query: None,
            explanation: String::new(),
            assumptions: vec![],
            clarification_question: None,
            structure_request: Some(names.iter().map(|name| name.to_string()).collect()),
        }
    }

    /// O pedido de estrutura vira a mensagem do modelo com o pedido e a do
    /// usuário com o que o app achou; o recorte e os tokens somam no context.
    #[test]
    fn structure_rounds_answer_the_request_in_order() {
        let prompt = prompts::for_db(DatabaseType::Postgres).unwrap();
        let found = RequestedStructure {
            text: "-- PostgreSQL. Schemas: \"sales\"\nCREATE TABLE \"sales\".\"orders\" (\n  \"id\" integer\n);\n-- Not found: \"ghost\"".into(),
            requested: 2,
            found: 1,
            missing: vec!["ghost".into()],
            truncated: true,
            tokens_estimate: 30,
        };
        let rounds = [
            Round::Structure(StructureRound {
                output: structure_request(&["sales.orders", "ghost"]),
                reply: StructureReply::Found(found),
            }),
            Round::Correction(Correction {
                output: output("SELECT nome FROM sales.orders"),
                stage: FailedStage::Preflight,
                class: FailureClass::UnknownObject,
                error_sanitized: None,
            }),
            Round::Structure(StructureRound {
                output: structure_request(&["sales.items"]),
                reply: StructureReply::LimitReached,
            }),
        ];
        let request = attempt_request(prompt, &schema(false), &[], "pedidos", &rounds, 200_000);

        let roles: Vec<Role> = request.messages.iter().map(|m| m.role).collect();
        assert_eq!(
            roles,
            [Role::System, Role::User, Role::Assistant, Role::User, Role::Assistant, Role::User, Role::Assistant, Role::User]
        );
        assert_eq!(
            request.messages[2].content,
            r#"{"kind":"structure_request","language":null,"query":null,"explanation":"","assumptions":[],"clarificationQuestion":null,"structureRequest":["sales.orders","ghost"]}"#
        );
        assert!(request.messages[3].content.starts_with("<requested_structure truncated=\"true\">\n-- PostgreSQL"));
        assert!(request.messages[3].content.contains("-- Not found: \"ghost\"\n</requested_structure>\nAnswer the question now"));
        assert!(request.messages[7].content.starts_with("<requested_structure limit_reached=\"true\">"));
        assert_eq!(
            request.context,
            AttemptContext {
                schema_truncated: true,
                schema_tokens_estimate: schema(false).tokens_estimate + 30,
                // A última rodada é de estrutura: não há falha antes desta
                previous_failure: None,
            }
        );
    }

    /// A resposta em texto vai no histórico como o modelo deu: o texto
    /// está no `explanation`.
    #[test]
    fn text_answers_go_to_the_history() {
        let prompt = prompts::for_db(DatabaseType::Postgres).unwrap();
        let history = [HistoryTurn {
            question: "o que é um índice?".into(),
            output: ModelOutput {
                kind: OutputKind::Answer,
                language: None,
                query: None,
                explanation: "Um índice acelera a busca.".into(),
                assumptions: vec![],
                clarification_question: None,
                structure_request: None,
            },
        }];
        let request = attempt_request(prompt, &schema(false), &history, "e um composto?", &[], 200_000);
        assert!(request.messages[2].content.contains(r#""kind":"answer""#));
        assert!(request.messages[2].content.contains("Um índice acelera a busca."));
    }

    #[test]
    fn request_serializes_as_the_contract() {
        let prompt = prompts::for_db(DatabaseType::Postgres).unwrap();
        let request = attempt_request(prompt, &schema(false), &[], "x", &[], 8_000);
        let value = serde_json::to_value(&request).unwrap();
        assert_eq!(value["promptVersion"], "postgres-v2");
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
