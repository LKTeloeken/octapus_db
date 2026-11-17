//! O cadastro de perguntas pela tela: lê e regrava o arquivo do conjunto
//! (`perguntas.toml`) e confere um gabarito nos bancos de teste.
//!
//! É a única coisa que a tela escreve, e só neste arquivo. As rodadas
//! continuam só para leitura: cada uma guarda a cópia do conjunto com que
//! começou, então editar o arquivo não muda rodada nenhuma.
//!
//! Uma pergunta só é gravada se passa na conferência do conjunto (formato,
//! idioma de cada texto, gabarito só-leitura). Conferir nos bancos é à parte
//! ([`check`]): executa cada gabarito duas vezes pelo caminho só-leitura e
//! compara os paradigmas entre si.

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::compare::Table;
use super::lab::{run_gold, Databases};
use super::questions::{self, RawFile, RawQuestion};
use super::{config, env_var};

/// Linhas de cada resultado que vão para a tela.
const PREVIEW_ROWS: usize = 50;

/// O arquivo do conjunto, ou um conjunto vazio se ele ainda não existe.
fn read(path: &Path) -> Result<RawFile, String> {
    match std::fs::read_to_string(path) {
        Ok(source) => questions::parse(&source),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(RawFile::default()),
        Err(error) => Err(format!("{}: {error}", path.display())),
    }
}

/// Regrava o arquivo inteiro de uma vez (temporário + renomear).
fn write(path: &Path, file: &RawFile) -> Result<(), String> {
    let text = questions::to_toml(file)?;
    let temporary = path.with_extension(format!("toml.{}.tmp", std::process::id()));
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder).map_err(|error| error.to_string())?;
    }
    std::fs::write(&temporary, text).map_err(|error| error.to_string())?;
    std::fs::rename(&temporary, path).map_err(|error| error.to_string())
}

/// Os problemas do conjunto, por pergunta (`None` = do arquivo).
fn problems(file: &RawFile) -> Vec<(Option<String>, String)> {
    let text = match questions::to_toml(file) {
        Ok(text) => text,
        Err(message) => return vec![(None, message)],
    };
    match questions::load(&text) {
        Ok(_) => Vec::new(),
        Err(problems) => problems.into_iter().map(|problem| (problem.question, problem.message)).collect(),
    }
}

/// O conjunto como está no arquivo, com os problemas de cada pergunta.
pub fn list(path: &Path) -> Result<Value, String> {
    let file = read(path)?;
    let problems: Vec<Value> = problems(&file)
        .into_iter()
        .map(|(question, message)| json!({ "question": question, "message": message }))
        .collect();
    Ok(json!({
        "path": path.display().to_string(),
        "version": file.version,
        "questions": file.question,
        "problems": problems,
    }))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Save {
    pub question: RawQuestion,
    /// O id que a pergunta tinha, numa edição (`None` = pergunta nova)
    pub replaces: Option<String>,
}

/// Grava uma pergunta nova ou editada. `Err` com os problemas dela, e nada é
/// gravado.
pub fn save(path: &Path, request: Save) -> Result<(), Vec<String>> {
    let Save { mut question, replaces } = request;
    question.id = question.id.trim().to_string();
    // Vazios não vão para o arquivo
    question.dataset = question.dataset.filter(|text| !text.trim().is_empty());
    question.source = question.source.filter(|text| !text.trim().is_empty());
    question.gold.retain(|_, gold| !gold.query.trim().is_empty());

    let mut file = read(path).map_err(|message| vec![message])?;
    let position = replaces
        .as_deref()
        .and_then(|old| file.question.iter().position(|existing| existing.id == old));
    let id = question.id.clone();
    match position {
        Some(index) => file.question[index] = question,
        None => file.question.push(question),
    }

    // Só os problemas desta pergunta impedem: outra pergunta com problema
    // não trava o cadastro
    let label = if id.is_empty() { format!("pergunta {}", position.unwrap_or(file.question.len() - 1) + 1) } else { id };
    let mine: Vec<String> = problems(&file)
        .into_iter()
        .filter(|(question, _)| question.as_deref().is_none_or(|question| question == label))
        .map(|(_, message)| message)
        .collect();
    if !mine.is_empty() {
        return Err(mine);
    }
    write(path, &file).map_err(|message| vec![message])
}

/// Tira uma pergunta do conjunto.
pub fn delete(path: &Path, id: &str) -> Result<(), String> {
    let mut file = read(path)?;
    let before = file.question.len();
    file.question.retain(|question| question.id != id);
    if file.question.len() == before {
        return Err(format!("a pergunta `{id}` não está no conjunto"));
    }
    write(path, &file)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Checked {
    paradigm: String,
    columns: Vec<String>,
    rows: Vec<Vec<Option<String>>>,
    row_count: usize,
}

impl Checked {
    fn new(paradigm: &str, table: &Table) -> Self {
        Self {
            paradigm: paradigm.to_string(),
            columns: table.columns.clone(),
            rows: table.rows.iter().take(PREVIEW_ROWS).cloned().collect(),
            row_count: table.rows.len(),
        }
    }
}

/// Executa os gabaritos de uma pergunta do conjunto nos bancos do ambiente
/// (duas vezes cada, só leitura) e devolve os resultados, os problemas e os
/// avisos — entre eles o de paradigmas com respostas diferentes.
pub async fn check(questions_path: &Path, environment_path: &Path, id: &str) -> Result<Value, String> {
    let mut file = read(questions_path)?;
    file.question.retain(|question| question.id == id);
    if file.question.is_empty() {
        return Err(format!("a pergunta `{id}` não está no conjunto (grave antes de conferir)"));
    }
    let set = questions::load(&questions::to_toml(&file)?)
        .map_err(|problems| problems.iter().map(|problem| problem.to_string()).collect::<Vec<_>>().join("; "))?;
    let source = std::fs::read_to_string(environment_path)
        .map_err(|error| format!("{}: {error}", environment_path.display()))?;
    let environment = config::load(&source)?;

    let databases = Databases::connect(&environment, &set, &[], false, &env_var).await?;
    let without_database: Vec<&String> = set.questions[0]
        .gold
        .keys()
        .filter(|paradigm| !environment.database.contains_key(*paradigm))
        .collect();
    let mut warnings: Vec<String> = without_database
        .iter()
        .map(|paradigm| format!("sem banco de `{paradigm}` no ambiente: gabarito não conferido"))
        .collect();

    Ok(match run_gold(&set, &[], &databases, environment.run.row_limit).await {
        Ok((gold, more)) => {
            warnings.extend(more);
            let results: Vec<Checked> = gold.iter().map(|result| Checked::new(&result.paradigm, &result.table)).collect();
            json!({ "ok": true, "results": results, "warnings": warnings, "problems": [] })
        }
        Err(problems) => {
            let problems: Vec<String> = problems.iter().map(|problem| problem.message.clone()).collect();
            json!({ "ok": false, "results": [], "warnings": warnings, "problems": problems })
        }
    })
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use super::*;
    use crate::experiment::questions::RawGold;
    use crate::experiment::sim;

    fn folder(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("octapus-experiment-editor-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn question(id: &str, query: &str) -> RawQuestion {
        RawQuestion {
            id: id.into(),
            difficulty: "simple".into(),
            pt: "Quantos clientes existem no cadastro da loja?".into(),
            en: "How many customers are there in the store records?".into(),
            dataset: Some(" ".into()),
            gold: BTreeMap::from([(
                "sqlite".to_string(),
                RawGold {
                    query: query.into(),
                    compare: None,
                },
            )]),
            ..Default::default()
        }
    }

    #[test]
    fn saves_edits_and_deletes_questions_in_the_file() {
        let path = folder("save").join("perguntas.toml");
        // O arquivo ainda não existe: conjunto vazio
        assert_eq!(list(&path).unwrap()["questions"], json!([]));

        let new = |question| Save {
            question,
            replaces: None,
        };
        save(&path, new(question("q1", "SELECT count(*) FROM customers"))).unwrap();
        save(&path, new(question("q2", "SELECT 2"))).unwrap();
        let listed = list(&path).unwrap();
        assert_eq!(listed["questions"].as_array().unwrap().len(), 2);
        assert_eq!(listed["problems"], json!([]));
        // O campo em branco não foi para o arquivo
        assert!(!std::fs::read_to_string(&path).unwrap().contains("dataset"));

        // Editar troca a pergunta no lugar, inclusive o id
        let mut edited = question("q1b", "SELECT count(*) FROM customers WHERE id > 0");
        edited.difficulty = "moderate".into();
        save(
            &path,
            Save {
                question: edited,
                replaces: Some("q1".into()),
            },
        )
        .unwrap();
        let set = questions::load(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let ids: Vec<&str> = set.questions.iter().map(|question| question.id.as_str()).collect();
        assert_eq!(ids, ["q1b", "q2"]);
        assert_eq!(set.questions[0].difficulty, "moderate");

        delete(&path, "q2").unwrap();
        assert!(delete(&path, "q2").unwrap_err().contains("não está no conjunto"));
        assert_eq!(list(&path).unwrap()["questions"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn a_question_with_problems_is_not_saved() {
        let path = folder("refuse").join("perguntas.toml");
        let new = |question| Save {
            question,
            replaces: None,
        };
        save(&path, new(question("q1", "SELECT 1"))).unwrap();
        let before = std::fs::read_to_string(&path).unwrap();

        // Gabarito que escreve, id repetido e texto no idioma errado
        let found = save(&path, new(question("q2", "DELETE FROM customers"))).unwrap_err();
        assert!(found.iter().any(|problem| problem.contains("não é de leitura")), "{found:?}");
        let found = save(&path, new(question("q1", "SELECT 1"))).unwrap_err();
        assert_eq!(found, ["`id` repetido"]);
        let mut wrong = question("q3", "SELECT 1");
        wrong.pt = wrong.en.clone();
        let found = save(&path, new(wrong)).unwrap_err();
        assert!(found[0].contains("o texto `pt` foi detectado"), "{found:?}");

        assert_eq!(std::fs::read_to_string(&path).unwrap(), before, "nada foi gravado");
    }

    /// Conferir nos bancos: os resultados de cada gabarito, e a recusa do que
    /// não roda — sempre só leitura.
    #[tokio::test]
    async fn checks_a_gold_against_the_databases() {
        let folder = folder("check");
        let (questions_path, environment_path) = (folder.join("perguntas.toml"), folder.join("ambiente.toml"));
        let database = folder.join("demo.sqlite");
        sim::create_demo_database(&database).unwrap();
        std::fs::write(&environment_path, sim::demo_environment(&database, 1)).unwrap();
        let new = |question| Save {
            question,
            replaces: None,
        };
        save(&questions_path, new(question("ok", "SELECT city, count(*) FROM customers GROUP BY city"))).unwrap();
        save(&questions_path, new(question("quebrada", "SELECT nope FROM customers"))).unwrap();

        let checked = check(&questions_path, &environment_path, "ok").await.unwrap();
        assert_eq!(checked["ok"], true);
        assert_eq!(checked["results"][0]["paradigm"], "sqlite");
        assert_eq!(checked["results"][0]["rowCount"], 3);
        assert_eq!(checked["results"][0]["columns"].as_array().unwrap().len(), 2);

        let checked = check(&questions_path, &environment_path, "quebrada").await.unwrap();
        assert_eq!(checked["ok"], false);
        assert!(checked["problems"][0].as_str().unwrap().contains("falhou"), "{checked}");

        assert!(check(&questions_path, &environment_path, "nenhuma").await.unwrap_err().contains("grave antes"));
    }
}
