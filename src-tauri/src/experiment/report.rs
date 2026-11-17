//! As saídas para a análise, calculadas dos testes gravados: o resumo por
//! grupo ([`summarize`]) e a tabela em formato longo, um teste por linha
//! ([`results_csv`]). A tela usa o mesmo resumo.
//!
//! Definições:
//! - **Acurácia na primeira tentativa**: testes que acertaram sem nenhuma
//!   correção, sobre o total do grupo.
//! - **Acurácia final**: testes que acertaram ao fim do turno, sobre o total.
//! - **Consultas inválidas**: das tentativas que geraram uma consulta, as que
//!   não rodaram até o fim (recusa do guard, erro no preflight ou na
//!   execução). Tentativa com saída fora do contrato é contada à parte.
//! - **Correspondência exata**: testes de PostgreSQL com gabarito cuja última
//!   consulta é igual à de referência; vazio nos outros paradigmas.
//! - **Tempo**: o tempo total do turno, da abertura ao encerramento.
//!
//! Para parear condições nos testes estatísticos, a chave é `question_id` +
//! `repetition`: cada combinação aparece uma vez por (paradigma, modelo,
//! idioma).

use std::collections::BTreeMap;

use serde::Serialize;

use super::record::UnitRecord;
use crate::assistant::output::OutputKind;

/// Por onde agrupar o resumo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dimension {
    Paradigm,
    Model,
    Language,
    Difficulty,
}

impl Dimension {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "paradigm" => Some(Self::Paradigm),
            "model" => Some(Self::Model),
            "language" => Some(Self::Language),
            "difficulty" => Some(Self::Difficulty),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Paradigm => "paradigm",
            Self::Model => "model",
            Self::Language => "language",
            Self::Difficulty => "difficulty",
        }
    }

    fn of(self, record: &UnitRecord) -> String {
        match self {
            Self::Paradigm => record.unit.paradigm.clone(),
            Self::Model => record.unit.model.clone(),
            Self::Language => record.unit.language.code().to_string(),
            Self::Difficulty => record.difficulty.clone(),
        }
    }
}

/// O resumo padrão: por paradigma, modelo e idioma.
pub const DEFAULT_DIMENSIONS: [Dimension; 3] = [Dimension::Paradigm, Dimension::Model, Dimension::Language];

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryRow {
    /// O valor de cada dimensão do agrupamento, na ordem pedida
    pub group: Vec<String>,
    pub tests: usize,
    pub unanswerable: usize,
    pub correct_first: usize,
    pub correct_final: usize,
    pub accuracy_first: f64,
    pub accuracy_final: f64,
    pub query_attempts: usize,
    pub invalid_queries: usize,
    /// `null` sem nenhuma consulta gerada
    pub invalid_query_rate: Option<f64>,
    pub invalid_outputs: usize,
    pub mean_attempts: f64,
    pub mean_turn_ms: f64,
    pub median_turn_ms: f64,
    /// Testes que entram na correspondência exata (PostgreSQL, com gabarito)
    pub exact_match_tests: usize,
    pub exact_matches: usize,
    pub exact_match_rate: Option<f64>,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost_usd: f64,
}

/// As contas de um teste que o resumo e a tabela longa dividem.
struct Counts {
    query_attempts: usize,
    invalid_queries: usize,
    invalid_outputs: usize,
    generation_ms: i64,
    input_tokens: i64,
    output_tokens: i64,
    cost_usd: f64,
}

fn counts(record: &UnitRecord) -> Counts {
    let mut counts = Counts {
        query_attempts: 0,
        invalid_queries: 0,
        invalid_outputs: 0,
        generation_ms: 0,
        input_tokens: 0,
        output_tokens: 0,
        cost_usd: 0.0,
    };
    for attempt in &record.attempts {
        if attempt.kind == Some(OutputKind::Query) {
            counts.query_attempts += 1;
            if attempt.failed_stage.is_some() {
                counts.invalid_queries += 1;
            }
        }
        if attempt.kind.is_none() && attempt.generation_error.as_deref() != Some("ASSISTANT_PROVIDER_ERROR") {
            counts.invalid_outputs += 1;
        }
        counts.generation_ms += attempt.generation_ms;
        if let Some(usage) = &attempt.usage {
            counts.input_tokens += usage.input_tokens + usage.cache_read_tokens + usage.cache_write_tokens;
            counts.output_tokens += usage.output_tokens;
            counts.cost_usd += usage.cost_usd;
        }
    }
    counts
}

/// O resumo dos testes, agrupado pelas dimensões, em ordem do grupo.
pub fn summarize(records: &[UnitRecord], dimensions: &[Dimension]) -> Vec<SummaryRow> {
    let mut groups: BTreeMap<Vec<String>, Vec<&UnitRecord>> = BTreeMap::new();
    for record in records {
        let group = dimensions.iter().map(|dimension| dimension.of(record)).collect();
        groups.entry(group).or_default().push(record);
    }

    groups
        .into_iter()
        .map(|(group, records)| {
            let tests = records.len();
            let rate = |part: usize, whole: usize| (whole > 0).then(|| part as f64 / whole as f64);
            let correct_first = records.iter().filter(|record| record.correct_first).count();
            let correct_final = records.iter().filter(|record| record.correct_final).count();
            let all: Vec<Counts> = records.iter().map(|record| counts(record)).collect();
            let query_attempts: usize = all.iter().map(|counts| counts.query_attempts).sum();
            let invalid_queries: usize = all.iter().map(|counts| counts.invalid_queries).sum();
            let exact: Vec<bool> = records.iter().filter_map(|record| record.exact_match).collect();
            let exact_matches = exact.iter().filter(|matched| **matched).count();

            let mut times: Vec<i64> = records.iter().map(|record| record.turn_ms).collect();
            times.sort_unstable();
            let median = match times.len() % 2 {
                1 => times[times.len() / 2] as f64,
                _ => (times[times.len() / 2 - 1] + times[times.len() / 2]) as f64 / 2.0,
            };

            SummaryRow {
                group,
                tests,
                unanswerable: records.iter().filter(|record| record.unanswerable).count(),
                correct_first,
                correct_final,
                accuracy_first: correct_first as f64 / tests as f64,
                accuracy_final: correct_final as f64 / tests as f64,
                query_attempts,
                invalid_queries,
                invalid_query_rate: rate(invalid_queries, query_attempts),
                invalid_outputs: all.iter().map(|counts| counts.invalid_outputs).sum(),
                mean_attempts: records.iter().map(|record| record.attempts.len()).sum::<usize>() as f64 / tests as f64,
                mean_turn_ms: times.iter().sum::<i64>() as f64 / tests as f64,
                median_turn_ms: median,
                exact_match_tests: exact.len(),
                exact_matches,
                exact_match_rate: rate(exact_matches, exact.len()),
                input_tokens: all.iter().map(|counts| counts.input_tokens).sum(),
                output_tokens: all.iter().map(|counts| counts.output_tokens).sum(),
                cost_usd: all.iter().map(|counts| counts.cost_usd).sum(),
            }
        })
        .collect()
}

/// O resumo em CSV, uma linha por grupo.
pub fn summary_csv(rows: &[SummaryRow], dimensions: &[Dimension]) -> String {
    let mut header: Vec<String> = dimensions.iter().map(|dimension| dimension.name().to_string()).collect();
    header.extend(
        [
            "tests",
            "unanswerable",
            "correct_first",
            "correct_final",
            "accuracy_first",
            "accuracy_final",
            "query_attempts",
            "invalid_queries",
            "invalid_query_rate",
            "invalid_outputs",
            "mean_attempts",
            "mean_turn_ms",
            "median_turn_ms",
            "exact_match_tests",
            "exact_matches",
            "exact_match_rate",
            "input_tokens",
            "output_tokens",
            "cost_usd",
        ]
        .map(String::from),
    );
    let mut csv = line(&header);
    for row in rows {
        let ratio = |value: Option<f64>| value.map(|value| format!("{value:.4}")).unwrap_or_default();
        let mut fields = row.group.clone();
        fields.extend([
            row.tests.to_string(),
            row.unanswerable.to_string(),
            row.correct_first.to_string(),
            row.correct_final.to_string(),
            format!("{:.4}", row.accuracy_first),
            format!("{:.4}", row.accuracy_final),
            row.query_attempts.to_string(),
            row.invalid_queries.to_string(),
            ratio(row.invalid_query_rate),
            row.invalid_outputs.to_string(),
            format!("{:.2}", row.mean_attempts),
            format!("{:.0}", row.mean_turn_ms),
            format!("{:.0}", row.median_turn_ms),
            row.exact_match_tests.to_string(),
            row.exact_matches.to_string(),
            ratio(row.exact_match_rate),
            row.input_tokens.to_string(),
            row.output_tokens.to_string(),
            format!("{:.6}", row.cost_usd),
        ]);
        csv.push_str(&line(&fields));
    }
    csv
}

/// A tabela em formato longo: um teste por linha.
pub fn results_csv(run_id: &str, records: &[UnitRecord]) -> String {
    let header = [
        "run_id",
        "question_id",
        "repetition",
        "paradigm",
        "model",
        "language",
        "difficulty",
        "unanswerable",
        "status",
        "error_code",
        "correct_first",
        "correct_final",
        "exact_match",
        "attempts",
        "counted_attempts",
        "query_attempts",
        "invalid_queries",
        "invalid_outputs",
        "first_failed_stage",
        "first_failure_class",
        "asked_user",
        "row_count",
        "mismatch",
        "turn_ms",
        "generation_ms",
        "input_tokens",
        "output_tokens",
        "cost_usd",
        "detected_language",
        "prompt_version",
        "conversation_id",
        "turn_id",
        "final_attempt_id",
        "started_at",
    ]
    .map(String::from);
    let mut csv = line(&header);
    for record in records {
        let counts = counts(record);
        let flag = |value: bool| if value { "1" } else { "0" }.to_string();
        let first_failure = record.attempts.iter().find(|attempt| attempt.failed_stage.is_some());
        let last = record.attempts.last();
        csv.push_str(&line(&[
            run_id.to_string(),
            record.unit.question_id.clone(),
            record.unit.repetition.to_string(),
            record.unit.paradigm.clone(),
            record.unit.model.clone(),
            record.unit.language.code().to_string(),
            record.difficulty.clone(),
            flag(record.unanswerable),
            record.status.clone(),
            record.error_code.clone().unwrap_or_default(),
            flag(record.correct_first),
            flag(record.correct_final),
            record.exact_match.map(flag).unwrap_or_default(),
            record.attempts.len().to_string(),
            record.counted_attempts.to_string(),
            counts.query_attempts.to_string(),
            counts.invalid_queries.to_string(),
            counts.invalid_outputs.to_string(),
            first_failure.and_then(|attempt| attempt.failed_stage.as_ref()).map(name).unwrap_or_default(),
            first_failure.and_then(|attempt| attempt.failure_class.as_ref()).map(name).unwrap_or_default(),
            flag(!record.asked_user.is_empty()),
            last.and_then(|attempt| attempt.row_count).map(|rows| rows.to_string()).unwrap_or_default(),
            record
                .comparison
                .as_ref()
                .and_then(|comparison| comparison.mismatch.as_ref())
                .map(name)
                .unwrap_or_default(),
            record.turn_ms.to_string(),
            counts.generation_ms.to_string(),
            counts.input_tokens.to_string(),
            counts.output_tokens.to_string(),
            format!("{:.6}", counts.cost_usd),
            name(&record.detected_language),
            record.prompt_version.clone().unwrap_or_default(),
            record.conversation_id.clone().unwrap_or_default(),
            record.turn_id.clone().unwrap_or_default(),
            last.map(|attempt| attempt.attempt_id.clone()).unwrap_or_default(),
            record.started_at.clone(),
        ]));
    }
    csv
}

/// O nome de um valor do contrato como vai no JSON (`unknown_object`…).
fn name<T: Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_default()
}

fn line(fields: &[String]) -> String {
    let mut line = fields.iter().map(|field| escape(field)).collect::<Vec<_>>().join(",");
    line.push('\n');
    line
}

/// Um campo de CSV (RFC 4180): entre aspas quando leva vírgula, aspas ou
/// quebra de linha.
fn escape(field: &str) -> String {
    match field.contains([',', '"', '\n', '\r']) {
        true => format!("\"{}\"", field.replace('"', "\"\"")),
        false => field.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assistant::language::{Detected, Language};
    use crate::experiment::plan::Unit;
    use crate::experiment::questions::Lang;
    use crate::experiment::record::AttemptRecord;
    use crate::experiment::unit::{blank, Base};
    use crate::models::AssistantStage;

    fn attempt(kind: Option<OutputKind>, failed: Option<AssistantStage>) -> AttemptRecord {
        AttemptRecord {
            attempt_id: "a".into(),
            index: 0,
            kind,
            query: None,
            output: None,
            raw: None,
            generation_error: None,
            counted: true,
            failed_stage: failed,
            failure_class: None,
            error: None,
            outcome: None,
            row_count: None,
            correct: None,
            exact_match: None,
            usage: None,
            generation_ms: 100,
        }
    }

    fn record(paradigm: &str, model: &str, first: bool, last: bool, turn_ms: i64, attempts: Vec<AttemptRecord>) -> UnitRecord {
        let unit = Unit {
            question_id: "q, \"1\"".into(),
            paradigm: paradigm.into(),
            language: Lang::Pt,
            model: model.into(),
            repetition: 1,
        };
        let mut record = blank(Base {
            unit: &unit,
            difficulty: "easy",
            dataset: None,
            unanswerable: false,
            question: "?",
            detected: Detected {
                language: Language::Pt,
                confidence: 1.0,
            },
        });
        record.status = "answered".into();
        record.correct_first = first;
        record.correct_final = last;
        record.turn_ms = turn_ms;
        record.exact_match = (paradigm == "postgres").then_some(first);
        record.attempts = attempts;
        record
    }

    fn sample() -> Vec<UnitRecord> {
        let query = || attempt(Some(OutputKind::Query), None);
        let failed = || attempt(Some(OutputKind::Query), Some(AssistantStage::Preflight));
        vec![
            record("postgres", "a", true, true, 1000, vec![query()]),
            record("postgres", "a", false, true, 3000, vec![failed(), query()]),
            record("postgres", "a", false, false, 8000, vec![failed(), attempt(None, None), failed()]),
            record("mongodb", "a", true, true, 2000, vec![query()]),
        ]
    }

    #[test]
    fn summarizes_by_group() {
        let rows = summarize(&sample(), &DEFAULT_DIMENSIONS);
        assert_eq!(rows.len(), 2);

        let mongo = &rows[0];
        assert_eq!(mongo.group, ["mongodb", "a", "pt"]);
        assert_eq!((mongo.tests, mongo.accuracy_final, mongo.exact_match_rate), (1, 1.0, None));

        let postgres = &rows[1];
        assert_eq!((postgres.tests, postgres.correct_first, postgres.correct_final), (3, 1, 2));
        assert!((postgres.accuracy_first - 1.0 / 3.0).abs() < 1e-9);
        assert!((postgres.accuracy_final - 2.0 / 3.0).abs() < 1e-9);
        // 5 consultas geradas, 3 não rodaram; a saída fora do contrato à parte
        assert_eq!((postgres.query_attempts, postgres.invalid_queries, postgres.invalid_outputs), (5, 3, 1));
        assert_eq!(postgres.invalid_query_rate, Some(0.6));
        assert_eq!((postgres.mean_turn_ms, postgres.median_turn_ms), (4000.0, 3000.0));
        assert_eq!((postgres.exact_match_tests, postgres.exact_matches), (3, 1));

        // Outro agrupamento com os mesmos testes
        let by_paradigm = summarize(&sample(), &[Dimension::Paradigm]);
        assert_eq!(by_paradigm.iter().map(|row| row.tests).collect::<Vec<_>>(), [1, 3]);
        assert!(summarize(&[], &DEFAULT_DIMENSIONS).is_empty());
    }

    #[test]
    fn csv_has_one_test_per_line_and_escapes_fields() {
        let records = sample();
        let csv = results_csv("rodada", &records);
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines.len(), 1 + records.len());
        assert!(lines[0].starts_with("run_id,question_id,repetition,paradigm,model,language,"));
        assert!(lines[1].starts_with("rodada,\"q, \"\"1\"\"\",1,postgres,a,pt,easy,0,answered,,1,1,1,1,"), "{}", lines[1]);
        assert!(lines[3].contains(",preflight,"), "{}", lines[3]);
        // Mesmo número de campos em todas as linhas
        // (o id da pergunta leva uma vírgula entre aspas)
        let header_fields = lines[0].split(',').count();
        assert_eq!(lines[4].split(',').count(), header_fields + 1);

        let summary = summary_csv(&summarize(&records, &DEFAULT_DIMENSIONS), &DEFAULT_DIMENSIONS);
        assert!(summary.starts_with("paradigm,model,language,tests,"));
        assert!(summary.contains("\npostgres,a,pt,3,0,1,2,0.3333,0.6667,5,3,0.6000,1,"), "{summary}");
        assert!(summary.contains("\nmongodb,a,pt,1,0,1,1,1.0000,1.0000,1,0,0.0000,0,1.00,2000,2000,0,0,,"), "{summary}");
    }
}
