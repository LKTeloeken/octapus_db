//! O ambiente de uma rodada: a API ([`Session`]), os bancos de teste
//! ([`Databases`]) e os gabaritos executados. O [`Lab`] monta cada teste como
//! o `assistant_turn_ask` monta um turno no app, com uma conversa nova (sem
//! histórico) e o modelo fixo, e o entrega ao `unit::run`.
//!
//! Tudo que toca um banco passa pelo caminho só-leitura do assistente (guard +
//! `execute_read_only`), inclusive o gabarito.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;

use super::compare::{compare, Table};
use super::config::{ApiConfig, AskUser, Environment, RunConfig};
use super::plan::Unit;
use super::questions::{db_type, Lang, Problem, QuestionSet};
use super::record::{now, GoldResult, LiveUnit, Stage, UnitRecord};
use super::round::Runner;
use super::setup::{conversation_schemas, has_catalog, query_language, sample_values_option, structure_context};
use super::unit::{self, Base, Expected, Job, Observer};
use crate::adapters::{create_adapter, create_catalog_source, DatabaseAdapter, ReadOnlyOp};
use crate::assistant::backend::{AttemptProgress, Generation, ModelBackend, ModelProfile};
use crate::assistant::intermediary::Intermediary;
use crate::assistant::journal::TurnJournal;
use crate::assistant::language::{self, Detected};
use crate::assistant::messages::AttemptRequest;
use crate::assistant::read_only::{self, AdapterDatabase};
use crate::assistant::requested::RequestedLookup;
use crate::assistant::schema_context::ContextRequest;
use crate::assistant::turn::{self, TurnPorts, TurnSetup};
use crate::assistant::{prompts, AssistantService, CancelFlag};
use crate::catalog::FLAT_SCHEMA;
use crate::error::{Error, Result};
use crate::models::{
    AssistantConversation, AssistantMe, AssistantModel, AssistantTurnStatus, AssistantTurnSummary, AttemptOutcome,
    DatabaseType, QueryOptions,
};
use crate::services::{connection_identity, CatalogEntry, CatalogService, SourceOpener};

/// Quanto esperar o catálogo de um banco sincronizar antes da rodada.
const CATALOG_SYNC_TIMEOUT: Duration = Duration::from_secs(180);

/// A conversa abriu com um modelo que não é o do teste.
const MODEL_NOT_FIXED: &str = "EXPERIMENT_MODEL_NOT_FIXED";

/// A API do experimento do ponto de vista da rodada: abre a conversa de cada
/// teste e ocupa as duas portas do turno. Um modelo local entra aqui depois,
/// com outra implementação.
#[async_trait]
pub trait Session: Send + Sync {
    /// Uma conversa nova do paradigma, com o modelo pedido; devolve a
    /// conversa como o app a lê antes do turno.
    async fn open(&self, paradigm: &str, model_id: &str, title: &str) -> Result<AssistantConversation>;
    fn backend(&self) -> &dyn ModelBackend;
    fn journal(&self) -> &dyn TurnJournal;
}

/// A API intermediária com o token do participante.
pub struct ApiSession {
    service: AssistantService,
    token: String,
}

impl ApiSession {
    /// Liga na API (a de `OCTAPUS_ASSISTANT_URL`, como o app) e consegue o
    /// token: o de `token_env` ou o convite de `invite_env`, trocado agora.
    pub async fn connect(api: &ApiConfig, env: &(dyn Fn(&str) -> Option<String> + Sync)) -> Result<Self> {
        let service = AssistantService::from_env();
        let (_, client) = service.endpoint()?;
        let read = |name: &Option<String>| name.as_deref().and_then(env).filter(|value| !value.is_empty());
        let token = match (read(&api.token_env), read(&api.invite_env)) {
            (Some(token), _) => token,
            (None, Some(invite)) => client.redeem(invite.trim(), Some("octapus experiment")).await?.token,
            (None, None) => {
                return Err(Error::InvalidState(
                    "the token/invite environment variable is not set".into(),
                ))
            }
        };
        Ok(Self { service, token })
    }

    pub fn url(&self) -> Option<&str> {
        self.service.base_url()
    }

    pub async fn me(&self) -> Result<AssistantMe> {
        self.service.endpoint()?.1.me(&self.token).await
    }

    pub async fn models(&self) -> Result<Vec<AssistantModel>> {
        self.service.endpoint()?.1.models(&self.token).await
    }

    pub async fn accept_consent(&self, version: &str) -> Result<()> {
        self.service.endpoint()?.1.consent_accept(&self.token, version).await
    }

    /// As duas portas do turno do app, sobre esta sessão.
    fn api(&self) -> Result<Intermediary<'_>> {
        let (_, client) = self.service.endpoint()?;
        Ok(Intermediary {
            client,
            token: &self.token,
            assistant: &self.service,
        })
    }
}

#[async_trait]
impl Session for ApiSession {
    async fn open(&self, paradigm: &str, model_id: &str, title: &str) -> Result<AssistantConversation> {
        let (_, client) = self.service.endpoint()?;
        let summary = client
            .create_conversation(&self.token, paradigm, Some(model_id), Some(title))
            .await?;
        self.api()?.conversation(&summary.id).await
    }

    fn backend(&self) -> &dyn ModelBackend {
        self
    }

    fn journal(&self) -> &dyn TurnJournal {
        self
    }
}

#[async_trait]
impl ModelBackend for ApiSession {
    async fn generate(
        &self,
        turn_id: &str,
        request: &AttemptRequest,
        cancel: &CancelFlag,
        progress: &(dyn Fn(AttemptProgress) + Send + Sync),
    ) -> Result<Generation> {
        self.api()?.generate(turn_id, request, cancel, progress).await
    }
}

#[async_trait]
impl TurnJournal for ApiSession {
    async fn conversation(&self, conversation_id: &str) -> Result<AssistantConversation> {
        self.api()?.conversation(conversation_id).await
    }

    async fn open_turn(
        &self,
        conversation_id: &str,
        question: &str,
        language: Detected,
        client_turn_id: &str,
    ) -> Result<AssistantTurnSummary> {
        self.api()?.open_turn(conversation_id, question, language, client_turn_id).await
    }

    async fn record_attempt(&self, turn_id: &str, request: &AttemptRequest, generation: &Generation) -> Result<()> {
        self.api()?.record_attempt(turn_id, request, generation).await
    }

    async fn report_outcome(&self, attempt_id: &str, outcome: &AttemptOutcome) -> Result<()> {
        self.api()?.report_outcome(attempt_id, outcome).await
    }

    async fn finish_turn(&self, turn_id: &str, status: AssistantTurnStatus) -> Result<()> {
        self.api()?.finish_turn(turn_id, status).await
    }
}

/// O banco de teste de um paradigma, aberto uma vez para a rodada inteira.
pub struct Database {
    pub db_type: DatabaseType,
    pub adapter: Arc<dyn DatabaseAdapter>,
    /// O catálogo (`None` no Redis, que não tem)
    pub entry: Option<Arc<CatalogEntry>>,
    /// Os schemas da conversa
    pub schemas: Vec<String>,
}

impl Database {
    /// A página da execução, como a do `assistant_turn_ask`.
    fn page(&self, row_limit: i64, query_id: Option<String>) -> QueryOptions {
        QueryOptions {
            limit: row_limit,
            schema: self.schemas.first().filter(|schema| schema.as_str() != FLAT_SCHEMA).cloned(),
            query_id,
            ..QueryOptions::default()
        }
    }

    /// Executa uma consulta de referência pelo caminho só-leitura do
    /// assistente: o guard recusa qualquer escrita antes de chegar ao banco.
    pub async fn read(&self, query: &str, row_limit: i64) -> Result<Table> {
        let op = ReadOnlyOp::new(query_language(self.db_type), query);
        let result = read_only::execute(self.adapter.as_ref(), self.db_type, &op, self.page(row_limit, None)).await?;
        Ok(Table::from(&result))
    }
}

/// Os bancos da rodada, por paradigma, e o serviço de catálogo deles (em
/// memória: nada vai para a pasta de dados do app).
pub struct Databases {
    pub catalogs: CatalogService,
    pub by_paradigm: BTreeMap<String, Database>,
}

impl Databases {
    /// Conecta nos bancos do ambiente e deixa o catálogo de cada um
    /// sincronizado, como num app com a conexão já aberta.
    pub async fn connect(
        environment: &Environment,
        env: &(dyn Fn(&str) -> Option<String> + Sync),
    ) -> std::result::Result<Self, String> {
        let catalogs = CatalogService::default();
        let mut by_paradigm = BTreeMap::new();

        for (server_id, (paradigm, config)) in environment.database.iter().enumerate() {
            let fail = |error: Error| format!("[database.{paradigm}]: {error}");
            let db_type = db_type(paradigm).ok_or_else(|| format!("[database.{paradigm}]: paradigma desconhecido"))?;
            let server = config
                .server(db_type, &|name| env(name))
                .map_err(|message| format!("[database.{paradigm}]: {message}"))?;
            let adapter = create_adapter(&server, &config.database).await.map_err(fail)?;
            adapter.test_connection().await.map_err(fail)?;

            let entry = match has_catalog(db_type) {
                true => {
                    let (source_server, database) = (server.clone(), config.database.clone());
                    let opener: SourceOpener = Arc::new(move || {
                        let (server, database) = (source_server.clone(), database.clone());
                        Box::pin(async move { create_catalog_source(&server, &database).await })
                    });
                    let entry = catalogs
                        .open(server_id as i64 + 1, &config.database, connection_identity(&server), opener)
                        .await;
                    catalogs.ensure_fresh(&entry, true);
                    let started = Instant::now();
                    while catalogs.status(&entry).syncing {
                        if started.elapsed() > CATALOG_SYNC_TIMEOUT {
                            return Err(format!("[database.{paradigm}]: o catálogo não sincronizou a tempo"));
                        }
                        tokio::time::sleep(Duration::from_millis(50)).await;
                    }
                    if catalogs.status(&entry).error.is_some() {
                        return Err(format!("[database.{paradigm}]: falha ao ler a estrutura do banco"));
                    }
                    Some(entry)
                }
                false => None,
            };
            log::info!("experimento: banco do paradigma {paradigm} conectado");
            by_paradigm.insert(
                paradigm.clone(),
                Database {
                    db_type,
                    adapter,
                    entry,
                    schemas: conversation_schemas(db_type, config.schemas.clone()),
                },
            );
        }
        Ok(Self { catalogs, by_paradigm })
    }
}

/// Executa cada gabarito duas vezes e confere: sem erro, cabe na página e dá
/// o mesmo resultado nas duas (pelas regras da própria pergunta). Devolve os
/// resultados esperados e os avisos, ou os problemas.
pub async fn run_gold(
    questions: &QuestionSet,
    only: &[String],
    databases: &Databases,
    row_limit: i64,
) -> std::result::Result<(Vec<GoldResult>, Vec<String>), Vec<Problem>> {
    let mut results = Vec::new();
    let mut warnings = Vec::new();
    let mut problems = Vec::new();

    for question in &questions.questions {
        if !only.is_empty() && !only.contains(&question.id) {
            continue;
        }
        for (paradigm, gold) in &question.gold {
            let Some(database) = databases.by_paradigm.get(paradigm) else {
                continue;
            };
            let mut problem = |message: String| {
                problems.push(Problem {
                    question: Some(question.id.clone()),
                    message: format!("gabarito `{paradigm}` {message}"),
                })
            };
            let first = match database.read(&gold.query, row_limit).await {
                Ok(table) => table,
                Err(error) => {
                    problem(format!("falhou: {error}"));
                    continue;
                }
            };
            if first.truncated {
                problem(format!("passa de {row_limit} linhas: restrinja a consulta ou aumente `row_limit`"));
                continue;
            }
            match database.read(&gold.query, row_limit).await {
                Ok(second) if compare(&first, &second, &gold.rules).equal => {}
                Ok(_) => {
                    problem("não é determinístico: duas execuções deram resultados diferentes".into());
                    continue;
                }
                Err(error) => {
                    problem(format!("falhou na segunda execução: {error}"));
                    continue;
                }
            }
            if first.rows.is_empty() {
                warnings.push(format!(
                    "{}: gabarito `{paradigm}` não devolve linhas (qualquer resultado vazio contaria como acerto)",
                    question.id
                ));
            }
            results.push(GoldResult {
                question_id: question.id.clone(),
                paradigm: paradigm.clone(),
                query: gold.query.clone(),
                table: first,
            });
        }
    }

    match problems.is_empty() {
        true => Ok((results, warnings)),
        false => Err(problems),
    }
}

/// O laboratório de uma rodada: monta e roda cada teste.
pub struct Lab<'a> {
    pub session: &'a dyn Session,
    pub databases: &'a Databases,
    pub questions: &'a QuestionSet,
    pub gold: &'a [GoldResult],
    pub run: &'a RunConfig,
    pub run_id: &'a str,
}

#[async_trait]
impl Runner for Lab<'_> {
    async fn run(&self, unit: &Unit, cancel: &CancelFlag, observe: Observer<'_>) -> UnitRecord {
        let question = self.questions.get(&unit.question_id);
        let text = question.map_or("", |question| question.text(unit.language));
        let mut record = unit::blank(Base {
            unit,
            difficulty: question.map_or("", |question| question.difficulty.as_str()),
            unanswerable: question.is_some_and(|question| question.unanswerable),
            question: text,
            detected: language::detect(text),
        });
        observe(&LiveUnit {
            key: record.key.clone(),
            unit: unit.clone(),
            question: record.question.clone(),
            difficulty: record.difficulty.clone(),
            stage: Stage::Preparing,
            started_at: record.started_at.clone(),
            attempts: Vec::new(),
        });

        match self.prepare_and_run(unit, record.clone(), cancel, observe).await {
            Ok(done) => done,
            Err((partial, error)) => {
                log::warn!("experimento: teste não abriu ({})", error.code());
                record = partial;
                record.error_code = Some(match &error {
                    Error::InvalidState(message) if message == MODEL_NOT_FIXED => MODEL_NOT_FIXED.to_string(),
                    other => other.code().to_string(),
                });
                record.finished_at = now();
                record
            }
        }
    }
}

impl Lab<'_> {
    /// Os mesmos passos do `assistant_turn_ask`, com a conversa nova de cada
    /// teste. `Err` = o teste não chegou ao turno (leva o registro até ali).
    async fn prepare_and_run(
        &self,
        unit: &Unit,
        mut record: UnitRecord,
        cancel: &CancelFlag,
        observe: Observer<'_>,
    ) -> std::result::Result<UnitRecord, (UnitRecord, Error)> {
        macro_rules! check {
            ($result:expr) => {{
                // Num `let`: o que a expressão pegou emprestado do registro
                // já foi solto quando ele sai junto com o erro
                let result = $result;
                match result {
                    Ok(value) => value,
                    Err(error) => return Err((record, error)),
                }
            }};
        }
        let missing = |what: &str| Error::InvalidState(format!("{what} is not part of this round"));
        let question = check!(self.questions.get(&unit.question_id).ok_or_else(|| missing("the question")));
        let database = check!(self.databases.by_paradigm.get(&unit.paradigm).ok_or_else(|| missing("the paradigm")));
        let db_type = database.db_type;
        let prompt = check!(prompts::for_db(db_type).ok_or_else(|| missing("the prompt")));
        record.prompt_version = Some(prompt.version.to_string());

        // Conversa nova: sem histórico de outro teste, com o modelo do teste
        let title = format!("tcc {} {}", self.run_id, unit.question_id);
        let conversation = check!(self.session.open(prompt.paradigm, &unit.model, &title).await);
        record.conversation_id = Some(conversation.summary.id.clone());
        record.model_info = Some(conversation.summary.model.clone());
        if conversation.summary.model.id != unit.model {
            return Err((record, Error::InvalidState(MODEL_NOT_FIXED.into())));
        }

        let sample_values = sample_values_option(db_type, self.run.mongo_sample_values).unwrap_or(false);
        let history = turn::history_from(&conversation);
        let past_questions: Vec<String> = history.iter().map(|turn| turn.question.clone()).collect();
        let model = ModelProfile::from(&conversation.summary.model);
        let schema = check!(
            structure_context(
                &self.databases.catalogs,
                database.entry.as_ref(),
                db_type,
                database.adapter.as_ref(),
                ContextRequest {
                    schemas: &database.schemas,
                    question: &record.question,
                    history: &past_questions,
                    context_window: model.context_window,
                    mentions: &[],
                    past_mentions: &[],
                    sample_values,
                },
            )
            .await
        );

        let client_turn_id = uuid_v4();
        let setup = TurnSetup {
            conversation_id: conversation.summary.id.clone(),
            client_turn_id: client_turn_id.clone(),
            language: language::detect(&record.question),
            question: record.question.clone(),
            prompt,
            db_type,
            expected_language: query_language(db_type),
            schema,
            history,
            model,
            // A execução automática fica ligada: é ela que dá o resultado
            auto_execute: true,
            sample_values: sample_values_option(db_type, sample_values),
            // Trocado pela política de pergunta ao usuário (`unit::run`)
            reply_timeout: turn::REPLY_TIMEOUT,
            page: database.page(self.run.row_limit, Some(client_turn_id)),
        };
        let lookup = RequestedLookup {
            catalogs: &self.databases.catalogs,
            entry: database.entry.clone(),
            adapter: Arc::clone(&database.adapter),
            db_type,
            scope: &database.schemas,
            sample_values,
        };
        let db = AdapterDatabase {
            adapter: Arc::clone(&database.adapter),
            db_type,
        };

        let expected = question.gold.get(&unit.paradigm).and_then(|gold| {
            let result = self
                .gold
                .iter()
                .find(|result| result.question_id == unit.question_id && result.paradigm == unit.paradigm)?;
            Some(Expected {
                gold: result,
                rules: gold.rules,
            })
        });
        if !question.unanswerable && expected.is_none() {
            return Err((record, missing("the gold result")));
        }

        let job = Job {
            record,
            ports: TurnPorts {
                backend: self.session.backend(),
                journal: self.session.journal(),
                db: &db,
                structure: &lookup,
            },
            setup,
            expected,
            ask_user: self.run.ask_user,
            reply: match (self.run.ask_user, &self.run.ask_user_reply) {
                (AskUser::Reply, Some(texts)) => Some(match unit.language {
                    Lang::Pt => texts.pt.as_str(),
                    Lang::En => texts.en.as_str(),
                }),
                _ => None,
            },
        };
        Ok(unit::run(job, cancel, observe).await)
    }
}

/// UUID v4 para a idempotência do turno (o app manda o do front).
fn uuid_v4() -> String {
    let bits: u128 = (rand::random::<u128>() & !(0xf << 76) | (0x4 << 76)) & !(0x3 << 62) | (0x2 << 62);
    let hex = format!("{bits:032x}");
    format!("{}-{}-{}-{}-{}", &hex[..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..])
}
