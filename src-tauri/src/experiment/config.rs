//! A configuração do ambiente de uma rodada (TOML): a API, os modelos, os
//! bancos de teste de cada paradigma e os parâmetros da rodada.
//!
//! ```toml
//! [api]
//! # url = "http://localhost:8787"    # sem isto, vale OCTAPUS_ASSISTANT_URL, como no app
//! invite_env = "OCTAPUS_E2E_INVITE"  # o NOME da variável com o convite
//! models = ["modelo-a", "modelo-b"]  # ids dos modelos na API
//!
//! [run]
//! repetitions = 1
//! seed = 42
//!
//! [database.postgres]
//! host = "localhost"
//! port = 5432
//! username = "postgres"
//! password_env = "OCTAPUS_EXPERIMENT_PG_PASSWORD"
//! database = "loja"
//!
//! [database.mongodb]
//! uri_env = "OCTAPUS_EXPERIMENT_MONGO_URI"
//! database = "loja"
//! ```
//!
//! Quando cada pergunta tem o seu banco (os do BIRD), `{dataset}` é trocado
//! pelo `dataset` da pergunta: `database = "{dataset}"` com um database por
//! conjunto (como o `./experiment/db.sh` carrega o PostgreSQL e o MongoDB), ou
//! `schemas = ["{dataset}"]` num PostgreSQL com um schema por conjunto.
//!
//! Senha, token, convite e URI de conexão nunca ficam no arquivo: ele só diz
//! em que variável de ambiente cada um está. Por isso o arquivo inteiro pode
//! ser guardado com a rodada.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::questions::{db_type, Lang};
use super::setup::FIRST_PAGE_ROWS;
use crate::models::{DatabaseType, Server, SslMode};

/// A variável de ambiente que o app lê para achar a API (`assistant/config.rs`).
pub const API_URL_ENV: &str = "OCTAPUS_ASSISTANT_URL";

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Environment {
    pub api: ApiConfig,
    #[serde(default)]
    pub run: RunConfig,
    pub database: BTreeMap<String, DatabaseConfig>,
}

/// Quem gera as tentativas. Hoje só a API intermediária; um modelo local entra
/// aqui, com uma implementação nova de [`Session`](super::lab::Session).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Backend {
    #[default]
    Intermediary,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApiConfig {
    #[serde(default)]
    pub backend: Backend,
    /// A URL da API; sem ela, vale a `OCTAPUS_ASSISTANT_URL` do ambiente
    pub url: Option<String>,
    /// Variável com o token de participante
    pub token_env: Option<String>,
    /// Variável com o convite, trocado por um token no começo da rodada
    pub invite_env: Option<String>,
    /// Os modelos da rodada, pelo id na API
    pub models: Vec<String>,
}

/// O que fazer quando o assistente pergunta algo ao usuário (`ask_user`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AskUser {
    /// Ninguém responde: o turno encerra como pedido de esclarecimento
    #[default]
    Clarify,
    /// Responde com o texto fixo de `ask_user_reply`, no idioma da pergunta
    Reply,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplyTexts {
    pub pt: String,
    pub en: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct RunConfig {
    pub repetitions: u32,
    pub languages: Vec<Lang>,
    pub seed: u64,
    /// Testes ao mesmo tempo; 1 = sequencial
    pub concurrency: usize,
    pub ask_user: AskUser,
    pub ask_user_reply: Option<ReplyTexts>,
    /// Valores de exemplo dos campos do MongoDB no contexto
    pub mongo_sample_values: bool,
    /// Linhas da página do resultado (a do app é 200)
    pub row_limit: i64,
    /// Testes seguidos com erro (não abriram ou falharam no meio) antes de
    /// pausar a rodada
    pub max_consecutive_errors: u32,
    /// Minutos de pausa depois de `max_consecutive_errors` erros seguidos,
    /// antes de tentar de novo; 0 = para direto
    pub pause_minutes: u64,
    /// Pausas por execução; na seguinte, a rodada para
    pub max_pauses: u32,
    /// Fração dos testes desta execução que pode terminar em `failed` antes
    /// de a rodada parar (0,10 = 10%), contada depois de `failure_rate_after`
    pub max_failure_rate: f64,
    pub failure_rate_after: usize,
    /// Gasto máximo de cada modelo, em dólares, pelo custo que a API mede
    /// (`{ "id-do-modelo" = 45.0 }`); modelo fora do mapa não tem teto
    pub max_cost_usd: BTreeMap<String, f64>,
}

impl Default for RunConfig {
    fn default() -> Self {
        Self {
            repetitions: 1,
            languages: vec![Lang::Pt, Lang::En],
            seed: 1,
            concurrency: 1,
            ask_user: AskUser::default(),
            ask_user_reply: None,
            mongo_sample_values: false,
            row_limit: FIRST_PAGE_ROWS,
            max_consecutive_errors: 3,
            pause_minutes: 10,
            max_pauses: 3,
            max_failure_rate: 0.10,
            failure_rate_after: 30,
            max_cost_usd: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DatabaseConfig {
    pub host: Option<String>,
    pub port: Option<u16>,
    pub username: Option<String>,
    /// Variável com a senha
    pub password_env: Option<String>,
    /// Variável com a URI de conexão (no lugar de host/porta/usuário/senha)
    pub uri_env: Option<String>,
    /// Arquivo do banco (SQLite)
    pub path: Option<String>,
    pub database: String,
    /// Os schemas da conversa (PostgreSQL); sem isto, `public`
    pub schemas: Option<Vec<String>>,
    /// O que entra no lugar de `{dataset}` para cada conjunto, quando não é o
    /// próprio nome: o Redis só tem bancos numerados (`california_schools =
    /// "0"`). Conjunto fora do mapa usa o nome.
    pub datasets: Option<BTreeMap<String, String>>,
    #[serde(default)]
    pub ssl_mode: SslMode,
}

/// Lê e confere o arquivo de ambiente.
pub fn load(source: &str) -> Result<Environment, String> {
    let environment: Environment = toml::from_str(source).map_err(|error| {
        let message = error.message();
        // Quem escreveu `password = "..."` precisa saber para onde ela vai
        let secret = ["`password`", "`token`", "`invite`", "`uri`"].iter().any(|field| message.contains(field));
        match secret {
            true => format!("{message}. Segredo não fica no arquivo: use `password_env`, `token_env`, `invite_env` ou `uri_env` com o nome da variável de ambiente"),
            false => message.to_string(),
        }
    })?;
    environment.check()?;
    Ok(environment)
}

impl Environment {
    fn check(&self) -> Result<(), String> {
        let fail = |message: &str| Err(message.to_string());
        if self.api.models.is_empty() || self.api.models.iter().any(|model| model.trim().is_empty()) {
            return fail("[api] models: liste ao menos um modelo, pelo id na API");
        }
        let mut models = self.api.models.clone();
        models.sort();
        models.dedup();
        if models.len() != self.api.models.len() {
            return fail("[api] models: modelo repetido");
        }
        if self.api.token_env.is_none() && self.api.invite_env.is_none() {
            return fail("[api]: informe `token_env` ou `invite_env` (o nome da variável de ambiente)");
        }

        let run = &self.run;
        if run.repetitions == 0 || run.concurrency == 0 || run.row_limit <= 0 || run.max_consecutive_errors == 0 {
            return fail("[run]: repetitions, concurrency, row_limit e max_consecutive_errors precisam ser maiores que zero");
        }
        let mut languages = run.languages.clone();
        languages.sort();
        languages.dedup();
        if languages.is_empty() || languages.len() != run.languages.len() {
            return fail("[run] languages: use \"pt\" e/ou \"en\", sem repetir");
        }
        if run.max_failure_rate.is_nan() || run.max_failure_rate <= 0.0 || run.max_failure_rate > 1.0 {
            return fail("[run] max_failure_rate: use uma fração entre 0 e 1 (0.10 = 10%)");
        }
        for (model, limit) in &run.max_cost_usd {
            if !self.api.models.contains(model) {
                return Err(format!("[run] max_cost_usd: `{model}` não está em [api] models"));
            }
            if limit.is_nan() || *limit <= 0.0 {
                return Err(format!("[run] max_cost_usd: o teto de `{model}` precisa ser maior que zero"));
            }
        }
        if run.ask_user == AskUser::Reply && run.ask_user_reply.is_none() {
            return fail("[run] ask_user = \"reply\" precisa de `ask_user_reply = { pt = \"...\", en = \"...\" }`");
        }

        if self.database.is_empty() {
            return fail("nenhum banco: configure ao menos um [database.<paradigma>]");
        }
        for (paradigm, database) in &self.database {
            let Some(db_type) = db_type(paradigm) else {
                return Err(format!("[database.{paradigm}]: paradigma desconhecido"));
            };
            let located = match db_type {
                DatabaseType::Sqlite => database.path.is_some(),
                _ => database.host.is_some() || database.uri_env.is_some(),
            };
            if !located {
                return Err(format!(
                    "[database.{paradigm}]: informe {}",
                    if db_type == DatabaseType::Sqlite { "`path`" } else { "`host` ou `uri_env`" }
                ));
            }
        }
        Ok(())
    }

    /// Os paradigmas com banco configurado.
    pub fn paradigms(&self) -> Vec<String> {
        self.database.keys().cloned().collect()
    }
}

/// O que a configuração de um banco troca pelo `dataset` da pergunta.
pub const DATASET: &str = "{dataset}";

impl DatabaseConfig {
    /// O banco muda com o `dataset` da pergunta (`{dataset}` em `database`,
    /// `schemas` ou `path`)?
    pub fn per_dataset(&self) -> bool {
        self.database.contains(DATASET)
            || self.path.as_deref().is_some_and(|path| path.contains(DATASET))
            || self.schemas.as_ref().is_some_and(|schemas| schemas.iter().any(|schema| schema.contains(DATASET)))
    }

    /// A configuração para as perguntas de um `dataset`.
    pub fn for_dataset(&self, dataset: Option<&str>) -> Result<Self, String> {
        if !self.per_dataset() {
            return Ok(self.clone());
        }
        let dataset = dataset.ok_or("o banco usa `{dataset}`, mas a pergunta não tem `dataset`")?;
        let dataset = self.datasets.as_ref().and_then(|map| map.get(dataset)).map_or(dataset, String::as_str);
        let fill = |text: &String| text.replace(DATASET, dataset);
        Ok(Self {
            database: fill(&self.database),
            path: self.path.as_ref().map(fill),
            schemas: self.schemas.as_ref().map(|schemas| schemas.iter().map(fill).collect()),
            ..self.clone()
        })
    }

    /// O servidor como o app o teria cadastrado, com os segredos lidos do
    /// ambiente. `env` é de onde ler (o do processo, fora dos testes).
    pub fn server(&self, db_type: DatabaseType, env: &dyn Fn(&str) -> Option<String>) -> Result<Server, String> {
        let read = |name: &Option<String>| match name {
            Some(name) => env(name)
                .filter(|value| !value.is_empty())
                .map(Some)
                .ok_or_else(|| format!("a variável de ambiente {name} não está definida")),
            None => Ok(None),
        };
        let connection_uri = match db_type {
            DatabaseType::Sqlite => self.path.clone(),
            _ => read(&self.uri_env)?,
        };
        Ok(Server {
            id: Some(1),
            name: "experimento".into(),
            db_type,
            host: self.host.clone().unwrap_or_else(|| "localhost".into()),
            port: self.port.unwrap_or(match db_type {
                DatabaseType::Postgres => 5432,
                DatabaseType::Mongodb => 27017,
                DatabaseType::Redis => 6379,
                _ => 0,
            }),
            username: self.username.clone().unwrap_or_default(),
            password: read(&self.password_env)?.unwrap_or_default(),
            default_database: None,
            ssl_mode: self.ssl_mode,
            ssl_root_cert: None,
            connection_uri,
            scope_databases: None,
            scope_schemas: None,
            group_id: None,
            assistant_sample_values: false,
            display_timezone: None,
            created_at: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID: &str = r#"
[api]
invite_env = "CONVITE"
models = ["modelo-a", "modelo-b"]

[run]
repetitions = 3
seed = 42

[database.postgres]
host = "localhost"
username = "postgres"
password_env = "SENHA_PG"
database = "loja"
schemas = ["public", "vendas"]

[database.sqlite]
path = "/tmp/loja.db"
database = "main"
"#;

    #[test]
    fn reads_the_environment_with_defaults() {
        let environment = load(VALID).unwrap();
        assert_eq!(environment.api.backend, Backend::Intermediary);
        assert_eq!(environment.paradigms(), ["postgres", "sqlite"]);

        let run = &environment.run;
        assert_eq!((run.repetitions, run.seed, run.concurrency), (3, 42, 1));
        assert_eq!(run.languages, [Lang::Pt, Lang::En]);
        // As decisões padrão: esclarecimento, sem valores de exemplo, página do app
        assert_eq!((run.ask_user, run.mongo_sample_values, run.row_limit), (AskUser::Clarify, false, 200));
        // Os controles de erro: 3 seguidos pausam 10 minutos, até 3 vezes
        assert_eq!((run.max_consecutive_errors, run.pause_minutes, run.max_pauses), (3, 10, 3));
        assert_eq!((run.max_failure_rate, run.failure_rate_after), (0.10, 30));
        assert!(run.max_cost_usd.is_empty());
    }

    #[test]
    fn secrets_come_from_the_environment_only() {
        let environment = load(VALID).unwrap();
        let postgres = &environment.database["postgres"];
        let env = |name: &str| (name == "SENHA_PG").then(|| "s3cret".to_string());
        let server = postgres.server(DatabaseType::Postgres, &env).unwrap();
        assert_eq!((server.password.as_str(), server.port, server.username.as_str()), ("s3cret", 5432, "postgres"));

        let missing = postgres.server(DatabaseType::Postgres, &|_| None).unwrap_err();
        assert_eq!(missing, "a variável de ambiente SENHA_PG não está definida");

        let sqlite = environment.database["sqlite"].server(DatabaseType::Sqlite, &|_| None).unwrap();
        assert_eq!(sqlite.connection_uri.as_deref(), Some("/tmp/loja.db"));

        // Segredo escrito no arquivo é recusado, com a saída
        let inline = VALID.replace("password_env = \"SENHA_PG\"", "password = \"1234\"");
        let error = load(&inline).unwrap_err();
        assert!(error.contains("`password`") && error.contains("password_env"), "{error}");
    }

    #[test]
    fn the_dataset_of_the_question_picks_the_database() {
        let source = VALID
            .replace("schemas = [\"public\", \"vendas\"]", "schemas = [\"{dataset}\"]")
            .replace("database = \"main\"", "database = \"{dataset}\"");
        let environment = load(&source).unwrap();

        let postgres = &environment.database["postgres"];
        assert!(postgres.per_dataset());
        let financial = postgres.for_dataset(Some("financial")).unwrap();
        assert_eq!((financial.database.as_str(), financial.schemas), ("loja", Some(vec!["financial".to_string()])));
        assert_eq!(environment.database["sqlite"].for_dataset(Some("financial")).unwrap().database, "financial");
        assert!(postgres.for_dataset(None).unwrap_err().contains("`dataset`"));

        // O Redis só tem bancos numerados: o mapa diz qual é o de cada conjunto
        let redis = load(&format!(
            "{source}\n[database.redis]\nhost = \"localhost\"\ndatabase = \"{{dataset}}\"\ndatasets = {{ financial = \"5\" }}\n"
        ))
        .unwrap()
        .database["redis"]
            .clone();
        assert_eq!(redis.for_dataset(Some("financial")).unwrap().database, "5");
        assert_eq!(redis.for_dataset(Some("outro")).unwrap().database, "outro");

        // Sem `{dataset}`, um banco só para todas as perguntas
        let fixed = &load(VALID).unwrap().database["postgres"];
        assert!(!fixed.per_dataset());
        assert_eq!(fixed.for_dataset(Some("financial")).unwrap(), *fixed);
    }

    #[test]
    fn rejects_incomplete_or_inconsistent_settings() {
        let check = |from: &str, to: &str| load(&VALID.replace(from, to)).unwrap_err();
        assert!(check("models = [\"modelo-a\", \"modelo-b\"]", "models = []").contains("models"));
        assert!(check("\"modelo-b\"", "\"modelo-a\"").contains("repetido"));
        assert!(check("invite_env = \"CONVITE\"\n", "").contains("token_env"));
        assert!(check("repetitions = 3", "repetitions = 0").contains("maiores que zero"));
        assert!(check("seed = 42", "seed = 42\nask_user = \"reply\"").contains("ask_user_reply"));
        assert!(check("seed = 42", "seed = 42\nlanguages = [\"pt\", \"pt\"]").contains("languages"));
        assert!(check("host = \"localhost\"\n", "").contains("`host` ou `uri_env`"));
        assert!(check("[database.sqlite]", "[database.oracle]").contains("paradigma desconhecido"));
        assert!(check("seed = 42", "seed = 42\nmax_failure_rate = 2.0").contains("max_failure_rate"));
        assert!(check("seed = 42", "seed = 42\nmax_cost_usd = { modelo-c = 10.0 }").contains("não está em [api] models"));
        assert!(check("seed = 42", "seed = 42\nmax_cost_usd = { modelo-a = 0.0 }").contains("maior que zero"));
        let limited = load(&VALID.replace("seed = 42", "seed = 42\nmax_cost_usd = { modelo-a = 45.0 }")).unwrap();
        assert_eq!(limited.run.max_cost_usd["modelo-a"], 45.0);

        let reply = VALID.replace(
            "seed = 42",
            "seed = 42\nask_user = \"reply\"\nask_user_reply = { pt = \"Use o bom senso.\", en = \"Use your judgment.\" }",
        );
        assert_eq!(load(&reply).unwrap().run.ask_user, AskUser::Reply);
    }
}
