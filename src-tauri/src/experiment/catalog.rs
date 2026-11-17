//! O catálogo de perguntas dos benchmarks baixados, de onde a tela de
//! cadastro tira as perguntas do experimento.
//!
//! Junta dois conjuntos pelo `question_id` do BIRD:
//! - **BIRD Mini-Dev** (`mini_dev_postgresql.json`): 500 perguntas com o SQL
//!   no dialeto do PostgreSQL e a dificuldade;
//! - **AptMQL-Bench** (`aptmqlbench_data.jsonl`, só o `dev`): as mesmas
//!   perguntas com o gabarito do MongoDB (`collection` + `aggregation_pipeline`),
//!   que aqui vira a operação do contrato do app (`mongo-operation.schema.json`).
//!
//! Cada gabarito passa pelo guard do assistente já na importação: o que ele
//! recusa fica marcado, para a tela mostrar antes de a pergunta ser cadastrada.
//! O catálogo é só referência (fica em `experiment/data/`, fora do git); o que
//! vale para a rodada é o conjunto de perguntas.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::config::Environment;
use super::lab::{gold_report, Databases};
use super::questions::{self, db_type, RawFile, RawGold, RawQuestion};
use super::setup::query_language;
use crate::adapters::ReadOnlyOp;
use crate::assistant::guard;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEntry {
    /// De onde veio: `bird:<question_id>`
    pub source: String,
    /// O banco da pergunta (o `db_id` do BIRD)
    pub dataset: String,
    /// A pergunta, em inglês, como está no benchmark
    pub question: String,
    /// A dica de conhecimento externo que o BIRD dá junto da pergunta
    pub evidence: String,
    /// A mesma dica com os campos do MongoDB (AptMQL)
    pub mongodb_evidence: Option<String>,
    pub difficulty: Option<String>,
    /// O gabarito em cada paradigma, na linguagem que o assistente gera
    pub gold: BTreeMap<String, String>,
    /// Gabaritos que o guard do assistente recusa, com o motivo
    pub problems: BTreeMap<String, String>,
    /// O texto do AptMQL, quando difere do Mini-Dev (o gabarito do MongoDB
    /// foi escrito para ele)
    pub other_question: Option<String>,
    /// O que a execução dos gabaritos nos bancos de teste mostrou ([`verify`])
    #[serde(default)]
    pub verified: Option<Verified>,
}

/// Os gabaritos de uma pergunta, executados nos bancos de teste.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Verified {
    /// Linhas do resultado de cada gabarito que rodou
    pub rows: BTreeMap<String, usize>,
    /// Gabaritos que não servem como estão: erro, resultado maior que a
    /// página ou diferente entre duas execuções
    pub problems: BTreeMap<String, String>,
    /// Os paradigmas deram a mesma resposta (`null` = menos de dois rodaram)
    pub agree: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    pub entries: Vec<CatalogEntry>,
}

#[derive(Deserialize)]
struct BirdRow {
    question_id: i64,
    db_id: String,
    question: String,
    #[serde(default)]
    evidence: String,
    #[serde(rename = "SQL")]
    sql: String,
    difficulty: Option<String>,
}

#[derive(Deserialize)]
struct AptRow {
    split: String,
    /// Número no `dev`, texto no `train`
    question_id: Value,
    db_id: String,
    question: String,
    #[serde(default)]
    evidence: String,
    mongodb_evidence: Option<String>,
    #[serde(rename = "MQL")]
    mql: AptQuery,
}

#[derive(Deserialize)]
struct AptQuery {
    collection: String,
    aggregation_pipeline: Vec<Value>,
}

/// Monta o catálogo dos dois arquivos (o JSON do Mini-Dev e o JSONL do AptMQL).
pub fn build(bird_json: &str, aptmql_jsonl: &str) -> Result<Catalog, String> {
    let bird: Vec<BirdRow> = serde_json::from_str(bird_json).map_err(|error| format!("BIRD Mini-Dev: {error}"))?;
    let mut entries: BTreeMap<(String, i64), CatalogEntry> = BTreeMap::new();

    for row in bird {
        let mut entry = CatalogEntry {
            source: format!("bird:{}", row.question_id),
            dataset: row.db_id.clone(),
            question: row.question.trim().to_string(),
            evidence: row.evidence.trim().to_string(),
            mongodb_evidence: None,
            difficulty: row.difficulty,
            gold: BTreeMap::new(),
            problems: BTreeMap::new(),
            other_question: None,
            verified: None,
        };
        add_gold(&mut entry, "postgres", row.sql.trim().to_string());
        entries.insert((row.db_id, row.question_id), entry);
    }

    for (number, line) in aptmql_jsonl.lines().enumerate().filter(|(_, line)| !line.trim().is_empty()) {
        let row: AptRow = serde_json::from_str(line).map_err(|error| format!("AptMQL, linha {}: {error}", number + 1))?;
        let (true, Some(question_id)) = (row.split == "dev", row.question_id.as_i64()) else {
            continue;
        };
        // A operação de leitura do contrato do app, com a ordem das chaves
        // do pipeline como veio (ela importa no `$sort` e no `$group`)
        let operation = json!({
            "collection": row.mql.collection,
            "operation": "aggregate",
            "pipeline": row.mql.aggregation_pipeline,
        });
        let query = serde_json::to_string_pretty(&operation).map_err(|error| error.to_string())?;
        let question = row.question.trim().to_string();
        let entry = entries.entry((row.db_id.clone(), question_id)).or_insert_with(|| CatalogEntry {
            source: format!("bird:{question_id}"),
            dataset: row.db_id.clone(),
            question: question.clone(),
            evidence: row.evidence.trim().to_string(),
            mongodb_evidence: None,
            difficulty: None,
            gold: BTreeMap::new(),
            problems: BTreeMap::new(),
            other_question: None,
            verified: None,
        });
        if entry.question != question {
            entry.other_question = Some(question);
        }
        entry.mongodb_evidence = row.mongodb_evidence.map(|evidence| evidence.trim().to_string());
        add_gold(entry, "mongodb", query);
    }

    Ok(Catalog {
        entries: entries.into_values().collect(),
    })
}

/// Executa os gabaritos do catálogo nos bancos do ambiente (duas vezes cada,
/// pelo caminho só-leitura) e anota em cada pergunta o que rodou e se os
/// paradigmas concordam. É o que deixa a tela filtrar as perguntas prontas
/// para o experimento. Não chama a API.
pub async fn verify(
    catalog: &mut Catalog,
    environment: &Environment,
    env: &(dyn Fn(&str) -> Option<String> + Sync),
) -> Result<(), String> {
    let id = |entry: &CatalogEntry| entry.source.replace(':', "_");
    let file = RawFile {
        question: catalog
            .entries
            .iter()
            .map(|entry| RawQuestion {
                id: id(entry),
                difficulty: entry.difficulty.clone().unwrap_or_else(|| "?".into()),
                pt: entry.question.clone(),
                en: entry.question.clone(),
                dataset: Some(entry.dataset.clone()),
                skip_language_check: true,
                gold: entry
                    .gold
                    .iter()
                    .filter(|(paradigm, _)| !entry.problems.contains_key(*paradigm) && environment.database.contains_key(*paradigm))
                    .map(|(paradigm, query)| (paradigm.clone(), RawGold { query: query.clone(), compare: None }))
                    .collect(),
                ..Default::default()
            })
            .filter(|question| !question.gold.is_empty())
            .collect(),
        ..Default::default()
    };
    let set = questions::load(&questions::to_toml(&file)?)
        .map_err(|problems| problems.iter().map(|problem| problem.to_string()).collect::<Vec<_>>().join("; "))?;
    let databases = Databases::connect(environment, &set, &[], false, env).await?;
    let report = gold_report(&set, &[], &databases, environment.run.row_limit).await;

    for entry in &mut catalog.entries {
        let id = id(entry);
        let mut verified = Verified::default();
        for result in report.results.iter().filter(|result| result.question_id == id) {
            verified.rows.insert(result.paradigm.clone(), result.table.rows.len());
        }
        for problem in report.problems.iter().filter(|problem| problem.question_id == id) {
            verified.problems.insert(problem.paradigm.clone(), problem.message.clone());
        }
        if verified.rows.len() > 1 {
            verified.agree = Some(!report.disagreements.iter().any(|found| found.question_id == id));
        }
        entry.verified = (!verified.rows.is_empty() || !verified.problems.is_empty()).then_some(verified);
    }
    Ok(())
}

/// Guarda o gabarito de um paradigma e anota se o guard do app o recusa.
fn add_gold(entry: &mut CatalogEntry, paradigm: &str, query: String) {
    if let Some(db_type) = db_type(paradigm) {
        let problem = match guard::inspect(&ReadOnlyOp::new(query_language(db_type), query.as_str()), db_type) {
            Ok(None) => None,
            Ok(Some(rejection)) => Some(format!("recusado pelo guard ({})", rejection.code())),
            Err(error) => Some(format!("inválido ({})", error.code())),
        };
        if let Some(problem) = problem {
            entry.problems.insert(paradigm.to_string(), problem);
        }
    }
    entry.gold.insert(paradigm.to_string(), query);
}

#[cfg(test)]
mod tests {
    use super::*;

    const BIRD: &str = r#"[
      {"question_id": 1312, "db_id": "student_club", "question": "What's Angela Sanders's major?",
       "evidence": "full name refers to first_name, last_name", "difficulty": "simple",
       "SQL": "SELECT T2.major_name FROM member AS T1 INNER JOIN major AS T2 ON T1.link_to_major = T2.major_id WHERE T1.first_name = 'Angela'"},
      {"question_id": 7, "db_id": "financial", "question": "Top four accounts?", "evidence": "", "difficulty": "moderate",
       "SQL": "SELECT account_id FROM account LIMIT 4"},
      {"question_id": 9, "db_id": "financial", "question": "Remove old loans", "evidence": "", "difficulty": "simple",
       "SQL": "DELETE FROM loan"}
    ]"#;

    const APTMQL: &str = r#"{"split":"dev","db_id":"student_club","question":"What's Angela Sanders's major?","evidence":"x","mongodb_evidence":"major refers to members.major.major_name","question_id":1312,"sample_id":"dev_1312","SQL":"...","MQL":{"collection":"members","aggregation_pipeline":[{"$match":{"first_name":"Angela"}},{"$sort":{"zeta":1,"alpha":-1}}]}}
{"split":"dev","db_id":"financial","question":"Top five accounts?","evidence":"","mongodb_evidence":"","question_id":7,"sample_id":"dev_7","SQL":"...","MQL":{"collection":"accounts","aggregation_pipeline":[{"$limit":5}]}}
{"split":"dev","db_id":"financial","question":"Only in AptMQL","evidence":"","mongodb_evidence":"","question_id":8,"sample_id":"dev_8","SQL":"...","MQL":{"collection":"accounts","aggregation_pipeline":[{"$out":"copy"}]}}
{"split":"train","db_id":"hockey","question":"Not dev","evidence":"","mongodb_evidence":"","question_id":"7","sample_id":"train_7","SQL":"...","MQL":{"collection":"x","aggregation_pipeline":[]}}
"#;

    #[test]
    fn joins_both_benchmarks_by_question_id() {
        let catalog = build(BIRD, APTMQL).unwrap();
        let sources: Vec<&str> = catalog.entries.iter().map(|entry| entry.source.as_str()).collect();
        // Por banco e id; o `train` do AptMQL fica de fora
        assert_eq!(sources, ["bird:7", "bird:8", "bird:9", "bird:1312"]);

        let both = &catalog.entries[3];
        assert_eq!((both.dataset.as_str(), both.difficulty.as_deref()), ("student_club", Some("simple")));
        assert_eq!(both.evidence, "full name refers to first_name, last_name");
        assert_eq!(both.mongodb_evidence.as_deref(), Some("major refers to members.major.major_name"));
        assert!(both.gold["postgres"].starts_with("SELECT T2.major_name"));
        assert!(both.problems.is_empty() && both.other_question.is_none());

        // O gabarito do MongoDB vira a operação do contrato do app, e a ordem
        // das chaves do `$sort` é a do benchmark
        let operation: Value = serde_json::from_str(&both.gold["mongodb"]).unwrap();
        assert_eq!(operation["collection"], "members");
        assert_eq!(operation["operation"], "aggregate");
        assert_eq!(operation["pipeline"][0], json!({ "$match": { "first_name": "Angela" } }));
        let sort = both.gold["mongodb"].find("zeta").unwrap() < both.gold["mongodb"].find("alpha").unwrap();
        assert!(sort, "{}", both.gold["mongodb"]);
    }

    /// A conferência nos bancos, com um catálogo sobre o SQLite da demonstração.
    #[tokio::test]
    async fn verifies_each_gold_on_the_databases() {
        use crate::experiment::{config, sim};

        let folder = std::env::temp_dir().join(format!("octapus-experiment-catalog-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).unwrap();
        let database = folder.join("demo.sqlite");
        sim::create_demo_database(&database).unwrap();
        let environment = config::load(&sim::demo_environment(&database, 1)).unwrap();

        let entry = |id: i64, query: &str| CatalogEntry {
            source: format!("bird:{id}"),
            dataset: "loja".into(),
            question: "How many customers are there?".into(),
            evidence: String::new(),
            mongodb_evidence: None,
            difficulty: None,
            gold: BTreeMap::from([("sqlite".to_string(), query.to_string())]),
            problems: BTreeMap::new(),
            other_question: None,
            verified: None,
        };
        let mut catalog = Catalog {
            entries: vec![
                entry(1, "SELECT count(*) FROM customers"),
                entry(2, "SELECT nope FROM customers"),
                entry(3, "SELECT id FROM orders"),
            ],
        };
        // Um gabarito que o guard já recusou não vai ao banco
        catalog.entries[2].problems.insert("sqlite".into(), "recusado pelo guard".into());
        verify(&mut catalog, &environment, &|_| None).await.unwrap();

        let ok = catalog.entries[0].verified.as_ref().unwrap();
        assert_eq!((ok.rows["sqlite"], ok.problems.len(), ok.agree), (1, 0, None));
        let broken = catalog.entries[1].verified.as_ref().unwrap();
        assert!(broken.rows.is_empty() && broken.problems["sqlite"].starts_with("falhou"), "{broken:?}");
        assert_eq!(catalog.entries[2].verified, None);
        let _ = std::fs::remove_dir_all(folder);
    }

    #[test]
    fn flags_text_differences_missing_gold_and_refused_gold() {
        let catalog = build(BIRD, APTMQL).unwrap();

        // Os dois conjuntos trazem textos diferentes: fica o do Mini-Dev
        let differs = &catalog.entries[0];
        assert_eq!((differs.question.as_str(), differs.other_question.as_deref()), ("Top four accounts?", Some("Top five accounts?")));

        // Só no AptMQL: sem gabarito de PostgreSQL nem dificuldade; o `$out` escreve
        let mongo_only = &catalog.entries[1];
        assert!(!mongo_only.gold.contains_key("postgres") && mongo_only.difficulty.is_none());
        assert!(mongo_only.problems["mongodb"].starts_with("recusado pelo guard"), "{:?}", mongo_only.problems);

        // Só no Mini-Dev, e não é leitura
        let write = &catalog.entries[2];
        assert!(!write.gold.contains_key("mongodb"));
        assert!(write.problems["postgres"].starts_with("recusado pelo guard"));

        assert!(build("{", "").unwrap_err().starts_with("BIRD Mini-Dev:"));
        assert!(build("[]", "{\"split\":").unwrap_err().starts_with("AptMQL, linha 1:"));
    }
}
