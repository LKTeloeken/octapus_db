//! O conjunto de perguntas do experimento (TOML) e a conferência que não
//! depende de banco nem de API: formato, idioma de cada texto e gabarito que
//! não seja de leitura.
//!
//! ```toml
//! version = "2026-11"                # rótulo livre do conjunto
//!
//! [defaults.compare]                 # regras de comparação (compare.rs)
//! decimals = 2
//!
//! [[question]]
//! id = "q001"
//! difficulty = "easy"
//! dataset = "student_club"           # opcional: o banco da pergunta
//! source = "bird:1312"               # opcional: de onde ela veio
//! pt = "Quantos clientes existem?"
//! en = "How many customers are there?"
//! [question.gold.postgres]
//! query = "SELECT count(*) FROM customers"
//! [question.gold.mongodb]
//! query = '''{ "collection": "customers", "operation": "countDocuments" }'''
//! compare = { columns = "position" } # só neste gabarito
//! [question.gold.redis]
//! same_as = "postgres"               # o resultado esperado é o do gabarito postgres
//!
//! [[question]]
//! id = "q050"
//! difficulty = "hard"
//! pt = "Qual é a cor preferida de cada cliente?"
//! en = "What is each customer's favorite color?"
//! unanswerable = true                # o acerto é pedir esclarecimento
//! ```
//!
//! O gabarito de cada paradigma é escrito na linguagem que o assistente gera
//! para aquele banco: SQL, a operação do Mongo ou o plano do Redis. Com
//! `same_as`, o paradigma não tem consulta própria: o resultado esperado é o
//! do gabarito de outro paradigma, o que só vale quando os dois bancos têm os
//! mesmos dados (o Redis carregado do PostgreSQL pelo `redis-load`).

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::compare::{ExtraColumns, Rules, RulesPatch};
use super::setup::query_language;
use crate::adapters::ReadOnlyOp;
use crate::assistant::language::{self, Language};
use crate::assistant::{guard, prompts};
use crate::models::DatabaseType;

/// Os bancos que o assistente atende, pelo nome do paradigma no contrato.
pub const PARADIGMS: [DatabaseType; 4] = [
    DatabaseType::Postgres,
    DatabaseType::Mongodb,
    DatabaseType::Redis,
    DatabaseType::Sqlite,
];

/// O banco de um paradigma (`postgres`, `mongodb`, `redis`, `sqlite`).
pub fn db_type(paradigm: &str) -> Option<DatabaseType> {
    PARADIGMS
        .into_iter()
        .find(|db_type| prompts::paradigm(*db_type) == Some(paradigm))
}

/// O idioma do texto de uma pergunta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    Pt,
    En,
}

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Self::Pt => "pt",
            Self::En => "en",
        }
    }

    fn detected(self) -> Language {
        match self {
            Self::Pt => Language::Pt,
            Self::En => Language::En,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gold {
    /// A consulta de referência (vazia com `same_as`)
    pub query: String,
    /// O paradigma de cujo gabarito vem o resultado esperado
    pub same_as: Option<String>,
    pub rules: Rules,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Question {
    pub id: String,
    pub difficulty: String,
    pub pt: String,
    pub en: String,
    /// Sem resposta possível: o acerto é o turno pedir esclarecimento
    pub unanswerable: bool,
    /// O banco da pergunta, quando o ambiente tem um por conjunto de dados
    pub dataset: Option<String>,
    /// De onde a pergunta veio (`bird:1312`)
    pub source: Option<String>,
    /// O gabarito de cada paradigma (vazio numa pergunta sem resposta)
    pub gold: BTreeMap<String, Gold>,
    /// Em que paradigmas a pergunta roda: os do gabarito, ou os listados em
    /// `paradigms`; vazio numa pergunta sem resposta = todos os da rodada
    pub paradigms: Vec<String>,
}

impl Question {
    pub fn text(&self, lang: Lang) -> &str {
        match lang {
            Lang::Pt => &self.pt,
            Lang::En => &self.en,
        }
    }

    pub fn runs_on(&self, paradigm: &str) -> bool {
        match self.unanswerable && self.paradigms.is_empty() {
            true => true,
            false => self.paradigms.iter().any(|name| name == paradigm),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestionSet {
    pub version: Option<String>,
    pub questions: Vec<Question>,
    /// O arquivo como foi lido, para a rodada guardar
    pub source: String,
    pub sha256: String,
}

impl QuestionSet {
    pub fn get(&self, id: &str) -> Option<&Question> {
        self.questions.iter().find(|question| question.id == id)
    }
}

/// Um problema do conjunto, com a pergunta quando é de uma.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub question: Option<String>,
    pub message: String,
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.question {
            Some(id) => write!(f, "{id}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

/// O arquivo como está escrito: é o que a tela de cadastro lê e regrava
/// ([`parse`] e [`to_toml`]).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "RawDefaults::is_empty")]
    pub defaults: RawDefaults,
    #[serde(default)]
    pub question: Vec<RawQuestion>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawDefaults {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compare: Option<RulesPatch>,
}

impl RawDefaults {
    fn is_empty(&self) -> bool {
        self.compare.is_none()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawQuestion {
    pub id: String,
    pub difficulty: String,
    pub pt: String,
    pub en: String,
    /// O banco da pergunta, quando o ambiente tem um por conjunto de dados
    /// (`{dataset}` no `ambiente.toml`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dataset: Option<String>,
    /// De onde a pergunta veio (`bird:1312`), para a rastreabilidade
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub unanswerable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paradigms: Option<Vec<String>>,
    /// Aceita o texto mesmo que o detector do app veja outro idioma
    #[serde(default, skip_serializing_if = "is_false")]
    pub skip_language_check: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compare: Option<RulesPatch>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub gold: BTreeMap<String, RawGold>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawGold {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub query: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub same_as: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compare: Option<RulesPatch>,
}

fn is_false(value: &bool) -> bool {
    !*value
}

/// Lê o arquivo sem conferir o conteúdo (só o formato).
pub fn parse(source: &str) -> Result<RawFile, String> {
    toml::from_str(source).map_err(|error| format!("formato inválido: {}", error.message()))
}

/// O arquivo de um conjunto, como a tela o regrava. Comentários escritos à
/// mão não sobrevivem: o arquivo é reescrito por inteiro.
pub fn to_toml(file: &RawFile) -> Result<String, String> {
    let body = toml::to_string_pretty(file).map_err(|error| error.to_string())?;
    Ok(format!(
        "# Conjunto de perguntas do experimento. Formato: src-tauri/src/experiment/questions.rs\n\
         # Regravado por inteiro pela tela de cadastro (comentários não são mantidos).\n\n{body}"
    ))
}

/// Lê e confere o conjunto. Devolve todos os problemas de uma vez.
pub fn load(source: &str) -> Result<QuestionSet, Vec<Problem>> {
    let raw = parse(source).map_err(|message| {
        vec![Problem {
            question: None,
            message,
        }]
    })?;

    let mut problems = Vec::new();
    let mut questions: Vec<Question> = Vec::new();
    if raw.question.is_empty() {
        problems.push(Problem {
            question: None,
            message: "nenhuma pergunta ([[question]])".into(),
        });
    }
    let defaults = Rules::default().with(raw.defaults.compare.as_ref());

    for (position, entry) in raw.question.into_iter().enumerate() {
        let id = entry.id.trim().to_string();
        let label = if id.is_empty() { format!("pergunta {}", position + 1) } else { id.clone() };
        let mut problem = |message: String| {
            problems.push(Problem {
                question: Some(label.clone()),
                message,
            })
        };

        let valid_id = !id.is_empty()
            && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
        if !valid_id {
            problem("`id` precisa ter só letras, números, `_` e `-`".into());
        } else if questions.iter().any(|question| question.id == id) {
            problem("`id` repetido".into());
        }
        if entry.difficulty.trim().is_empty() {
            problem("`difficulty` vazio".into());
        }
        let dataset = entry.dataset.as_deref().map(str::trim).filter(|name| !name.is_empty());
        if dataset.is_some_and(|name| !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')) {
            problem("`dataset` precisa ter só letras, números, `_` e `-`".into());
        }

        for (lang, text) in [(Lang::Pt, &entry.pt), (Lang::En, &entry.en)] {
            if text.trim().is_empty() {
                problem(format!("texto `{}` vazio", lang.code()));
                continue;
            }
            // O mesmo detector do app: é o idioma que vai gravado no turno
            let detected = language::detect(text.trim());
            if detected.language != lang.detected() && !entry.skip_language_check {
                problem(format!(
                    "o texto `{}` foi detectado como {:?} (confiança {:.2}); reescreva ou marque `skip_language_check = true`",
                    lang.code(),
                    detected.language,
                    detected.confidence
                ));
            }
        }

        let mut gold = BTreeMap::new();
        for (paradigm, raw_gold) in entry.gold {
            let Some(db_type) = db_type(&paradigm) else {
                problem(format!("paradigma desconhecido no gabarito: `{paradigm}`"));
                continue;
            };
            let query = raw_gold.query.trim().to_string();
            let mut rules = defaults.with(entry.compare.as_ref()).with(raw_gold.compare.as_ref());
            // A saída de um `foreach` do Redis sempre começa pela coluna do
            // item (a chave ou o id): o gabarito do Redis aceita colunas a
            // mais, salvo se a pergunta ou o gabarito disserem outra coisa
            let explicit = [entry.compare.as_ref(), raw_gold.compare.as_ref()]
                .iter()
                .any(|patch| patch.is_some_and(|patch| patch.extra_columns.is_some()));
            if db_type == DatabaseType::Redis && !explicit {
                rules.extra_columns = ExtraColumns::Allow;
            }
            if let Some(target) = raw_gold.same_as.as_deref().map(str::trim) {
                if !query.is_empty() {
                    problem(format!("gabarito `{paradigm}`: use `query` ou `same_as`, não os dois"));
                } else if target == paradigm || self::db_type(target).is_none() {
                    problem(format!("gabarito `{paradigm}`: `same_as` precisa citar outro paradigma"));
                } else {
                    gold.insert(paradigm, Gold { query, same_as: Some(target.to_string()), rules });
                }
                continue;
            }
            if query.is_empty() {
                problem(format!("gabarito `{paradigm}` vazio"));
                continue;
            }
            // O guard do assistente: gabarito que não é só leitura é recusado
            match guard::inspect(&ReadOnlyOp::new(query_language(db_type), query.as_str()), db_type) {
                Ok(None) => {}
                Ok(Some(rejection)) => {
                    problem(format!("gabarito `{paradigm}` recusado, não é de leitura ({})", rejection.code()));
                    continue;
                }
                Err(error) => {
                    problem(format!("gabarito `{paradigm}` inválido ({})", error.code()));
                    continue;
                }
            }
            gold.insert(paradigm, Gold { query, same_as: None, rules });
        }
        // `same_as` aponta para um gabarito com consulta própria
        let borrowed: Vec<(String, String)> = gold
            .iter()
            .filter_map(|(paradigm, found)| found.same_as.clone().map(|target| (paradigm.clone(), target)))
            .collect();
        for (paradigm, target) in borrowed {
            if gold.get(&target).is_none_or(|found| found.same_as.is_some()) {
                problem(format!("gabarito `{paradigm}`: `same_as = \"{target}\"`, mas `{target}` não tem consulta própria"));
                gold.remove(&paradigm);
            }
        }

        if entry.unanswerable && !gold.is_empty() {
            problem("pergunta sem resposta (`unanswerable`) não leva gabarito".into());
        }
        if !entry.unanswerable && gold.is_empty() {
            problem("sem gabarito válido (ou marque `unanswerable = true`)".into());
        }

        let mut paradigms: Vec<String> = match entry.paradigms {
            Some(listed) => {
                for name in &listed {
                    if db_type(name).is_none() {
                        problem(format!("paradigma desconhecido em `paradigms`: `{name}`"));
                    } else if !entry.unanswerable && !gold.contains_key(name) {
                        problem(format!("`paradigms` cita `{name}`, que não tem gabarito"));
                    }
                }
                listed
            }
            None => gold.keys().cloned().collect(),
        };
        paradigms.sort();
        paradigms.dedup();

        questions.push(Question {
            id,
            difficulty: entry.difficulty.trim().to_string(),
            pt: entry.pt.trim().to_string(),
            en: entry.en.trim().to_string(),
            unanswerable: entry.unanswerable,
            dataset: dataset.map(str::to_string),
            source: entry.source.clone(),
            gold,
            paradigms,
        });
    }

    if !problems.is_empty() {
        return Err(problems);
    }
    Ok(QuestionSet {
        version: raw.version,
        questions,
        source: source.to_string(),
        sha256: hex::encode(Sha256::digest(source.as_bytes())),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::experiment::compare::{ColumnMatch, RowOrder};

    const VALID: &str = r#"
version = "teste"

[defaults.compare]
decimals = 2

[[question]]
id = "q001"
difficulty = "easy"
pt = "Quantos clientes existem no cadastro da loja?"
en = "How many customers are there in the store records?"
compare = { order = "strict" }
[question.gold.postgres]
query = "SELECT count(*) FROM customers"
[question.gold.sqlite]
query = "SELECT count(*) FROM customers"
compare = { columns = "position", order = "ignore" }

[[question]]
id = "q002"
difficulty = "hard"
pt = "Qual é a cor preferida de cada um dos nossos clientes?"
en = "What is the favorite color of each one of our customers?"
unanswerable = true
"#;

    fn problems(source: &str) -> Vec<String> {
        load(source).unwrap_err().iter().map(Problem::to_string).collect()
    }

    #[test]
    fn loads_questions_and_layers_the_rules() {
        let set = load(VALID).unwrap();
        assert_eq!(set.version.as_deref(), Some("teste"));
        assert_eq!(set.sha256.len(), 64);
        assert_eq!(set.source, VALID);

        let first = set.get("q001").unwrap();
        assert_eq!(first.text(Lang::En), "How many customers are there in the store records?");
        assert_eq!(first.paradigms, ["postgres", "sqlite"]);
        assert!(first.runs_on("sqlite") && !first.runs_on("redis"));
        // arquivo → pergunta → gabarito
        let postgres = &first.gold["postgres"].rules;
        assert_eq!((postgres.decimals, postgres.order, postgres.columns), (2, RowOrder::Strict, ColumnMatch::Any));
        let sqlite = &first.gold["sqlite"].rules;
        assert_eq!((sqlite.decimals, sqlite.order, sqlite.columns), (2, RowOrder::Ignore, ColumnMatch::Position));

        let second = set.get("q002").unwrap();
        assert!(second.unanswerable && second.gold.is_empty());
        assert!(second.runs_on("redis"), "sem resposta roda em todos os paradigmas da rodada");
    }

    /// O que a tela regrava é lido de volta igual, com as consultas em
    /// várias linhas.
    #[test]
    fn the_rewritten_file_reads_back_the_same() {
        let mut raw = parse(VALID).unwrap();
        raw.question[0].dataset = Some("student_club".into());
        raw.question[0].source = Some("bird:1312".into());
        raw.question[0].gold.get_mut("postgres").unwrap().query =
            "SELECT count(*)\nFROM customers\nWHERE name = 'Ana \"A\"'".into();

        let written = to_toml(&raw).unwrap();
        assert_eq!(parse(&written).unwrap(), raw, "{written}");
        let set = load(&written).unwrap();
        let first = set.get("q001").unwrap();
        assert_eq!((first.dataset.as_deref(), first.source.as_deref()), (Some("student_club"), Some("bird:1312")));
        assert_eq!(first.gold["postgres"].query, "SELECT count(*)\nFROM customers\nWHERE name = 'Ana \"A\"'");
        // O que não foi preenchido não aparece no arquivo
        assert!(!written.contains("unanswerable = false") && !written.contains("skip_language_check"), "{written}");
        assert!(written.contains("unanswerable = true"));

        assert!(load(&VALID.replace("id = \"q001\"", "id = \"q001\"\ndataset = \"a b\"")).is_err());
    }

    #[test]
    fn rejects_bad_format() {
        assert!(problems("[[question]]\nid = 1")[0].starts_with("formato inválido"));
        // Campo que não existe (erro de digitação) não passa calado
        let typo = VALID.replace("difficulty = \"easy\"", "difficulty = \"easy\"\ndificuldade = \"x\"");
        assert!(problems(&typo)[0].contains("dificuldade"));
        assert_eq!(problems("version = \"x\""), ["nenhuma pergunta ([[question]])"]);
    }

    #[test]
    fn rejects_write_gold_and_wrong_language() {
        let write = VALID.replace("SELECT count(*) FROM customers\"\n[question.gold.sqlite]", "DELETE FROM customers\"\n[question.gold.sqlite]");
        let found = problems(&write);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].starts_with("q001: gabarito `postgres` recusado, não é de leitura"), "{found:?}");

        let swapped = VALID.replace(
            "en = \"How many customers are there in the store records?\"",
            "en = \"Quantos clientes existem no cadastro da nossa loja?\"",
        );
        let found = problems(&swapped);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].contains("o texto `en` foi detectado como Pt"), "{found:?}");
        // O autor pode assumir o texto
        let skipped = swapped.replace("id = \"q001\"", "id = \"q001\"\nskip_language_check = true");
        assert!(load(&skipped).is_ok());
    }

    #[test]
    fn reports_every_problem_at_once() {
        let source = r#"
[[question]]
id = "tem espaço"
difficulty = ""
pt = "Quantos clientes existem no cadastro da loja?"
en = ""

[[question]]
id = "q2"
difficulty = "easy"
pt = "Quantos pedidos foram feitos no ano passado pelos clientes?"
en = "How many orders were placed last year by the customers?"
unanswerable = true
[question.gold.oracle]
query = "SELECT 1"
[question.gold.postgres]
query = "SELECT 1"
"#;
        let found = problems(source);
        let expected = [
            "tem espaço: `id` precisa ter só letras, números, `_` e `-`",
            "tem espaço: `difficulty` vazio",
            "tem espaço: texto `en` vazio",
            "tem espaço: sem gabarito válido (ou marque `unanswerable = true`)",
            "q2: paradigma desconhecido no gabarito: `oracle`",
            "q2: pergunta sem resposta (`unanswerable`) não leva gabarito",
        ];
        assert_eq!(found, expected);
    }

    /// O Redis carregado do PostgreSQL usa o resultado do gabarito de lá, e
    /// aceita a coluna do item que o `foreach` sempre põe na frente.
    #[test]
    fn same_as_borrows_the_result_of_another_gold() {
        let borrowed = VALID.replace(
            "[question.gold.sqlite]",
            "[question.gold.redis]\nsame_as = \"postgres\"\n[question.gold.sqlite]",
        );
        let set = load(&borrowed).unwrap();
        let first = set.get("q001").unwrap();
        let redis = &first.gold["redis"];
        assert_eq!((redis.query.as_str(), redis.same_as.as_deref()), ("", Some("postgres")));
        assert_eq!(redis.rules.extra_columns, ExtraColumns::Allow);
        assert_eq!(first.gold["postgres"].rules.extra_columns, ExtraColumns::Forbid);
        assert_eq!(first.paradigms, ["postgres", "redis", "sqlite"]);
        // O que a tela regrava volta igual
        assert_eq!(parse(&to_toml(&parse(&borrowed).unwrap()).unwrap()).unwrap(), parse(&borrowed).unwrap());

        // A pergunta pode pedir o contrário
        let strict = borrowed.replace("same_as = \"postgres\"", "same_as = \"postgres\"\ncompare = { extra_columns = \"forbid\" }");
        assert_eq!(load(&strict).unwrap().get("q001").unwrap().gold["redis"].rules.extra_columns, ExtraColumns::Forbid);

        let to = |target: &str| borrowed.replace("same_as = \"postgres\"", &format!("same_as = \"{target}\""));
        assert_eq!(problems(&to("redis")), ["q001: gabarito `redis`: `same_as` precisa citar outro paradigma"]);
        assert_eq!(problems(&to("mongodb")), ["q001: gabarito `redis`: `same_as = \"mongodb\"`, mas `mongodb` não tem consulta própria"]);
        let both = borrowed.replace("same_as = \"postgres\"", "same_as = \"postgres\"\nquery = \"x\"");
        assert_eq!(problems(&both), ["q001: gabarito `redis`: use `query` ou `same_as`, não os dois"]);
    }

    #[test]
    fn rejects_duplicate_ids_and_paradigms_without_gold() {
        let duplicate = format!("{VALID}\n{}", VALID.split("[[question]]").nth(2).map(|q| format!("[[question]]{q}")).unwrap());
        assert!(problems(&duplicate).contains(&"q002: `id` repetido".to_string()));

        let listed = VALID.replace("compare = { order = \"strict\" }", "paradigms = [\"postgres\", \"redis\"]");
        assert_eq!(problems(&listed), ["q001: `paradigms` cita `redis`, que não tem gabarito"]);
    }
}
