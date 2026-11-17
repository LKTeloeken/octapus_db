//! Uma API simulada, sem rede: o modelo responde por um roteiro e o registro
//! só devolve ids. Serve ao teste do fluxo completo e ao comando `demo`, que
//! roda uma rodada inteira num SQLite descartável para ver a tela funcionando
//! sem gastar cota.
//!
//! Só a API é simulada. O turno, o guard, o preflight, a execução só-leitura,
//! o catálogo e o comparador são os de verdade.

use std::collections::{HashMap, VecDeque};
use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;
use parking_lot::Mutex;
use serde_json::{json, Value};

use super::lab::Session;
use super::questions::{db_type, QuestionSet};
use super::setup::query_language;
use crate::assistant::backend::{AttemptProgress, Generation, ModelBackend, StopReason};
use crate::assistant::journal::TurnJournal;
use crate::assistant::language::Detected;
use crate::assistant::messages::AttemptRequest;
use crate::assistant::{AssistantCode, AssistantError, CancelFlag};
use crate::error::{Error, Result};
use crate::models::{
    AssistantConversation, AssistantConversationSummary, AssistantModel, AssistantModelSelection,
    AssistantTurnStatus, AssistantTurnSummary, AssistantUsage, AttemptOutcome,
};

/// Tentativas por turno, como o limite da API (com as correções).
const TURN_LIMIT: usize = 4;

/// O que o modelo roteirizado responde numa tentativa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reply {
    Query(String),
    Clarification(String),
    AskUser(String),
    /// Texto fora do contrato
    Invalid(String),
}

/// O roteiro: as respostas de um turno, dados a pergunta, o modelo e o idioma.
pub type Script = Box<dyn Fn(&str, &str, &str) -> Vec<Reply> + Send + Sync>;

struct Turn {
    paradigm: String,
    replies: VecDeque<Reply>,
    generated: usize,
}

#[derive(Default)]
struct State {
    next_id: u64,
    /// Conversa → (paradigma, modelo)
    conversations: HashMap<String, (String, AssistantModel)>,
    turns: HashMap<String, Turn>,
}

pub struct SimSession {
    /// Texto da pergunta → (id, idioma)
    questions: HashMap<String, (String, &'static str)>,
    script: Script,
    /// Quanto cada tentativa "demora" a gerar
    delay: Duration,
    state: Mutex<State>,
    /// O que o turno mandou em cada pedido de tentativa
    pub requests: Mutex<Vec<AttemptRequest>>,
    /// Os outcomes e os encerramentos que o turno registrou
    pub outcomes: Mutex<Vec<(String, AttemptOutcome)>>,
    pub finished: Mutex<Vec<AssistantTurnStatus>>,
}

impl SimSession {
    pub fn new(questions: &QuestionSet, script: Script, delay: Duration) -> Self {
        let mut by_text = HashMap::new();
        for question in &questions.questions {
            by_text.insert(question.pt.clone(), (question.id.clone(), "pt"));
            by_text.insert(question.en.clone(), (question.id.clone(), "en"));
        }
        Self {
            questions: by_text,
            script,
            delay,
            state: Mutex::default(),
            requests: Mutex::default(),
            outcomes: Mutex::default(),
            finished: Mutex::default(),
        }
    }

    fn id(&self, kind: &str) -> String {
        let mut state = self.state.lock();
        state.next_id += 1;
        format!("sim-{kind}-{}", state.next_id)
    }
}

/// Os modelos da API simulada.
pub fn model(id: &str) -> AssistantModel {
    AssistantModel {
        id: id.to_string(),
        alias: id.to_string(),
        context_window: 200_000,
    }
}

const NOW: &str = "2026-01-01T00:00:00Z";

#[async_trait]
impl Session for SimSession {
    async fn open(&self, paradigm: &str, model_id: &str, title: &str) -> Result<AssistantConversation> {
        let id = self.id("conv");
        let model = model(model_id);
        self.state.lock().conversations.insert(id.clone(), (paradigm.to_string(), model.clone()));
        Ok(AssistantConversation {
            summary: AssistantConversationSummary {
                id,
                paradigm: paradigm.to_string(),
                model,
                selection_mode: AssistantModelSelection::Free,
                title: Some(title.to_string()),
                created_at: NOW.into(),
                updated_at: NOW.into(),
            },
            turns: Vec::new(),
        })
    }

    fn backend(&self) -> &dyn ModelBackend {
        self
    }

    fn journal(&self) -> &dyn TurnJournal {
        self
    }
}

#[async_trait]
impl TurnJournal for SimSession {
    async fn conversation(&self, _: &str) -> Result<AssistantConversation> {
        Err(Error::UnsupportedType("the simulated API keeps no conversations".into()))
    }

    async fn open_turn(
        &self,
        conversation_id: &str,
        question: &str,
        language: Detected,
        _: &str,
    ) -> Result<AssistantTurnSummary> {
        let id = self.id("turn");
        let mut state = self.state.lock();
        let (paradigm, model) = state
            .conversations
            .get(conversation_id)
            .cloned()
            .ok_or_else(|| Error::NotFound("conversation".into()))?;
        let (question_id, code) = self
            .questions
            .get(question)
            .cloned()
            .ok_or_else(|| Error::NotFound("question".into()))?;
        let replies = (self.script)(&question_id, &model.id, code).into();
        state.turns.insert(
            id.clone(),
            Turn {
                paradigm,
                replies,
                generated: 0,
            },
        );
        Ok(AssistantTurnSummary {
            id,
            index: 0,
            question: question.to_string(),
            language: language.language,
            status: AssistantTurnStatus::Open,
            created_at: NOW.into(),
        })
    }

    async fn record_attempt(&self, _: &str, _: &AttemptRequest, _: &Generation) -> Result<()> {
        Ok(())
    }

    async fn report_outcome(&self, attempt_id: &str, outcome: &AttemptOutcome) -> Result<()> {
        self.outcomes.lock().push((attempt_id.to_string(), outcome.clone()));
        Ok(())
    }

    async fn finish_turn(&self, _: &str, status: AssistantTurnStatus) -> Result<()> {
        self.finished.lock().push(status);
        Ok(())
    }
}

#[async_trait]
impl ModelBackend for SimSession {
    async fn generate(
        &self,
        turn_id: &str,
        request: &AttemptRequest,
        cancel: &CancelFlag,
        progress: &(dyn Fn(AttemptProgress) + Send + Sync),
    ) -> Result<Generation> {
        self.requests.lock().push(request.clone());
        let attempt_id = self.id("att");
        let (reply, index, paradigm) = {
            let mut state = self.state.lock();
            let turn = state.turns.get_mut(turn_id).ok_or_else(|| Error::NotFound("turn".into()))?;
            // Como a API: além do limite de tentativas, o turno é recusado
            let reply = match turn.generated < TURN_LIMIT {
                true => turn.replies.pop_front(),
                false => None,
            };
            let Some(reply) = reply else {
                return Err(AssistantError::new(AssistantCode::TurnLimitExceeded, "turn limit").into());
            };
            turn.generated += 1;
            (reply, turn.generated as i64 - 1, turn.paradigm.clone())
        };

        progress(AttemptProgress::Started {
            attempt_id: attempt_id.clone(),
            index,
        });
        tokio::select! {
            _ = tokio::time::sleep(self.delay) => {}
            _ = cancel.cancelled() => return Err(Error::Cancelled),
        }

        let language = db_type(&paradigm).map(query_language);
        let output = |kind: &str, query: Option<&str>, question: Option<&str>| {
            json!({
                "kind": kind,
                "language": query.and(language),
                "query": query,
                "explanation": "Resposta do modelo simulado.",
                "assumptions": [],
                "clarificationQuestion": question,
                "structureRequest": null,
                "choices": null,
            })
        };
        let result: std::result::Result<Value, (AssistantError, String)> = match reply {
            Reply::Query(query) => Ok(output("query", Some(&query), None)),
            Reply::Clarification(question) => Ok(output("clarification", None, Some(&question))),
            Reply::AskUser(question) => Ok(output("ask_user", None, Some(&question))),
            Reply::Invalid(raw) => Err((AssistantError::new(AssistantCode::InvalidOutput, "invalid output"), raw)),
        };
        let (result, raw) = match result {
            Ok(value) => {
                let raw = value.to_string();
                (Ok(value), raw)
            }
            Err((error, raw)) => (Err(error), raw),
        };
        Ok(Generation {
            attempt_id,
            index,
            result,
            usage: Some(AssistantUsage {
                input_tokens: 1200 + 150 * index,
                output_tokens: 40 + raw.len() as i64 / 4,
                cache_read_tokens: 0,
                cache_write_tokens: 0,
                cost_usd: 0.0,
            }),
            raw,
            stop: Some(StopReason::End),
            recorded: true,
        })
    }
}

// ── A rodada de demonstração ────────────────────────────────────────────────

pub const DEMO_MODELS: [&str; 2] = ["sim-a", "sim-b"];

/// O conjunto de perguntas da demonstração (e do teste do fluxo).
pub const DEMO_QUESTIONS: &str = r#"version = "demo"

[[question]]
id = "total_clientes"
difficulty = "easy"
pt = "Quantos clientes existem no cadastro da loja?"
en = "How many customers are there in the store records?"
[question.gold.sqlite]
query = "SELECT count(*) FROM customers"

[[question]]
id = "clientes_por_cidade"
difficulty = "medium"
pt = "Quantos clientes existem em cada cidade do cadastro?"
en = "How many customers are there in each city of the records?"
[question.gold.sqlite]
query = "SELECT city, count(*) FROM customers GROUP BY city"

[[question]]
id = "maiores_pedidos"
difficulty = "medium"
pt = "Quais são os três pedidos de maior valor, do maior para o menor?"
en = "Which are the three orders with the highest value, from highest to lowest?"
compare = { order = "strict" }
[question.gold.sqlite]
query = "SELECT id, total FROM orders ORDER BY total DESC, id LIMIT 3"

[[question]]
id = "ticket_medio"
difficulty = "medium"
pt = "Qual é o valor médio dos pedidos que já foram pagos pelos clientes?"
en = "What is the average value of the orders that have already been paid by the customers?"
compare = { decimals = 2 }
[question.gold.sqlite]
query = "SELECT avg(total) FROM orders WHERE status = 'paid'"

[[question]]
id = "faturamento_por_cliente"
difficulty = "hard"
pt = "Qual é o faturamento total de pedidos pagos de cada cliente, pelo nome dele?"
en = "What is the total revenue of paid orders for each customer, by the customer name?"
[question.gold.sqlite]
query = """
SELECT c.name, sum(o.total)
FROM orders o JOIN customers c ON c.id = o.customer_id
WHERE o.status = 'paid'
GROUP BY c.name
"""

[[question]]
id = "cor_preferida"
difficulty = "hard"
pt = "Qual é a cor preferida de cada um dos clientes da loja?"
en = "What is the favorite color of each one of the store customers?"
unanswerable = true
"#;

/// O roteiro da demonstração: um caso de cada tipo, com o `sim-b` errando
/// mais que o `sim-a`.
pub fn demo_script() -> Script {
    Box::new(|question, model, language| {
        let query = |sql: &str| Reply::Query(sql.to_string());
        let weaker = model == "sim-b";
        match question {
            // Acerto direto
            "total_clientes" => vec![query("SELECT count(*) AS total\nFROM customers")],
            // Coluna que não existe: o preflight recusa e a correção acerta
            "clientes_por_cidade" => vec![
                query("SELECT cidade, count(*) FROM customers GROUP BY cidade"),
                query("SELECT count(*) AS clientes, city\nFROM customers\nGROUP BY city"),
            ],
            // Consulta que roda, com o resultado errado (a ordem)
            "maiores_pedidos" if weaker => vec![query("SELECT id, total FROM orders ORDER BY total LIMIT 3")],
            "maiores_pedidos" => vec![query("SELECT id, total\nFROM orders\nORDER BY total DESC, id\nLIMIT 3")],
            // Saída fora do contrato e depois o acerto (arredondado)
            "ticket_medio" => vec![
                Reply::Invalid("Claro! A média é dada por SELECT avg(total)...".into()),
                query("SELECT round(avg(total), 2) AS media\nFROM orders\nWHERE status = 'paid'"),
            ],
            // Escrita (o guard recusa) e depois só consultas inválidas
            "faturamento_por_cliente" if weaker || language == "en" => vec![
                query("DELETE FROM orders WHERE status <> 'paid'"),
                query("SELECT nome, sum(total) FROM orders GROUP BY nome"),
                query("SELECT name, sum(total) FROM orders GROUP BY name"),
                query("SELECT customer, sum(total) FROM orders GROUP BY customer"),
                query("SELECT 1"),
            ],
            "faturamento_por_cliente" => vec![query(
                "SELECT c.name, sum(o.total) AS faturamento\nFROM orders o\nJOIN customers c ON c.id = o.customer_id\nWHERE o.status = 'paid'\nGROUP BY c.name",
            )],
            // Sem resposta: esclarecimento, pergunta ao usuário, ou um chute
            "cor_preferida" if weaker && language == "en" => vec![query("SELECT name, city FROM customers")],
            "cor_preferida" if weaker => vec![Reply::AskUser("Em que tabela fica a cor preferida?".into())],
            "cor_preferida" => vec![Reply::Clarification("O cadastro não tem a cor preferida dos clientes.".into())],
            _ => vec![Reply::Clarification("Pergunta fora do roteiro.".into())],
        }
    })
}

/// Cria o banco SQLite da demonstração.
pub fn create_demo_database(path: &Path) -> std::result::Result<(), String> {
    let _ = std::fs::remove_file(path);
    let connection = rusqlite::Connection::open(path).map_err(|error| error.to_string())?;
    connection
        .execute_batch(
            "CREATE TABLE customers (id INTEGER PRIMARY KEY, name TEXT NOT NULL, city TEXT NOT NULL);
             CREATE TABLE orders (
                 id INTEGER PRIMARY KEY,
                 customer_id INTEGER NOT NULL REFERENCES customers (id),
                 total REAL NOT NULL,
                 status TEXT NOT NULL
             );
             INSERT INTO customers (id, name, city) VALUES
                 (1, 'Ana', 'Porto Alegre'), (2, 'Bruno', 'Porto Alegre'), (3, 'Carla', 'Curitiba'),
                 (4, 'Diego', 'Recife'), (5, 'Elisa', 'Curitiba'), (6, 'Fábio', 'Porto Alegre');
             INSERT INTO orders (id, customer_id, total, status) VALUES
                 (1, 1, 120.50, 'paid'), (2, 1, 80.00, 'paid'), (3, 2, 310.25, 'paid'),
                 (4, 3, 45.90, 'cancelled'), (5, 3, 999.99, 'paid'), (6, 4, 15.00, 'paid'),
                 (7, 5, 250.00, 'pending'), (8, 5, 72.35, 'paid'), (9, 6, 640.10, 'paid'),
                 (10, 6, 33.33, 'cancelled');",
        )
        .map_err(|error| error.to_string())
}

/// O ambiente da demonstração, com o banco em `database`.
pub fn demo_environment(database: &Path, concurrency: usize) -> String {
    format!(
        "# Rodada simulada: sem rede, modelo e registro roteirizados\n\
         [api]\ntoken_env = \"SEM_REDE\"\nmodels = [\"{}\", \"{}\"]\n\n\
         [run]\nseed = 7\nconcurrency = {concurrency}\n\n\
         [database.sqlite]\npath = {:?}\ndatabase = \"main\"\n",
        DEMO_MODELS[0],
        DEMO_MODELS[1],
        database.to_string_lossy()
    )
}
