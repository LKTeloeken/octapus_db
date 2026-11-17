//! O fluxo completo de uma rodada, sem rede: o modelo e o registro são os
//! simulados (`sim.rs`); o turno, o guard, o preflight, a execução só-leitura,
//! o catálogo e o comparador são os de verdade, sobre um SQLite descartável.

use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

use super::compare::Mismatch;
use super::config::AskUser;
use super::lab::{run_gold, Databases, Lab};
use super::record::{GoldResult, UnitRecord};
use super::round::{self, Options};
use super::sim::{self, Reply, Script, SimSession};
use super::store::{Live, RoundStatus, RunDir};
use super::{report, Inputs};
use crate::assistant::failure::FailureClass;
use crate::assistant::output::OutputKind;
use crate::assistant::CancelFlag;
use crate::models::{AssistantStage, AssistantTurnStatus};

/// Uma rodada de demonstração montada numa pasta temporária.
struct Bench {
    path: PathBuf,
    dir: RunDir,
    inputs: Inputs,
    databases: Databases,
    gold: Vec<GoldResult>,
}

impl Bench {
    async fn new(name: &str, environment_extra: &str, only: &[&str]) -> Self {
        let path = std::env::temp_dir().join(format!("octapus-experiment-flow-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let dir = RunDir::create(&path).unwrap();
        std::fs::write(path.join("manifest.json"), "{}").unwrap();
        let database = path.join("demo.sqlite");
        sim::create_demo_database(&database).unwrap();

        let environment = sim::demo_environment(&database, 1).replace("seed = 7\n", &format!("seed = 7\n{environment_extra}"));
        let only = only.iter().map(|id| id.to_string()).collect();
        let inputs = Inputs::new(sim::DEMO_QUESTIONS, &environment, only).unwrap();
        let databases = Databases::connect(&inputs.environment, &|_| None).await.unwrap();
        let (gold, warnings) = run_gold(&inputs.questions, &inputs.only, &databases, inputs.environment.run.row_limit)
            .await
            .unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        Self {
            path,
            dir,
            inputs,
            databases,
            gold,
        }
    }

    fn session(&self, script: Script) -> SimSession {
        SimSession::new(&self.inputs.questions, script, Duration::ZERO)
    }

    /// Roda o que falta; `stop_after` interrompe depois de tantos testes.
    async fn round(&self, session: &SimSession, stop_after: Option<usize>) -> round::Outcome {
        let lab = Lab {
            session,
            databases: &self.databases,
            questions: &self.inputs.questions,
            gold: &self.gold,
            run: &self.inputs.environment.run,
            run_id: "fluxo",
        };
        let options = Options {
            concurrency: 1,
            max_consecutive_errors: 3,
            redo_failed: false,
        };
        let stop = CancelFlag::default();
        let ran = std::sync::atomic::AtomicUsize::new(0);
        let report = |_: &UnitRecord, _: usize, _: usize| {
            if Some(ran.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1) == stop_after {
                stop.cancel();
            }
        };
        round::run(&lab, &self.inputs.units, &self.dir, "fluxo", &options, &stop, &report).await.unwrap()
    }

    async fn orders(&self) -> Option<String> {
        self.databases.by_paradigm["sqlite"].read("SELECT count(*) FROM orders", 10).await.unwrap().rows[0][0].clone()
    }
}

impl Drop for Bench {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn find<'a>(records: &'a [UnitRecord], question: &str, model: &str, language: &str) -> &'a UnitRecord {
    records
        .iter()
        .find(|record| {
            record.unit.question_id == question && record.unit.model == model && record.unit.language.code() == language
        })
        .unwrap_or_else(|| panic!("sem o teste {question} {model} {language}"))
}

#[tokio::test]
async fn full_round_records_every_kind_of_outcome() {
    let bench = Bench::new("full", "", &[]).await;
    let session = bench.session(sim::demo_script());
    let outcome = bench.round(&session, None).await;

    // 6 perguntas × 2 modelos × 2 idiomas, cada uma numa conversa nova
    assert_eq!((outcome.status, outcome.total, outcome.done, outcome.ran), (RoundStatus::Finished, 24, 24, 24));
    let records = bench.dir.units().unwrap();
    assert_eq!(records.iter().map(|record| record.key.clone()).collect::<HashSet<_>>().len(), 24);
    assert_eq!(records.iter().filter_map(|record| record.conversation_id.clone()).collect::<HashSet<_>>().len(), 24);
    assert_eq!(session.finished.lock().len(), 24, "todo turno foi encerrado no registro");

    // Acerto direto
    let direct = find(&records, "total_clientes", "sim-a", "pt");
    assert_eq!(direct.status, "answered");
    assert!(direct.correct_first && direct.correct_final);
    assert_eq!((direct.attempts.len(), direct.counted_attempts), (1, 1));
    let attempt = &direct.attempts[0];
    assert_eq!((attempt.kind, attempt.row_count, attempt.correct), (Some(OutputKind::Query), Some(1), Some(true)));
    assert_eq!(attempt.query.as_deref(), Some("SELECT count(*) AS total\nFROM customers"));
    assert!(attempt.attempt_id.starts_with("sim-att-") && attempt.usage.is_some());
    assert!(attempt.outcome.as_ref().is_some_and(|outcome| outcome.execution.ok == Some(true) && outcome.auto_execute));
    assert_eq!(direct.result.as_ref().unwrap().rows, [[Some("6".to_string())]]);
    assert_eq!(direct.prompt_version.as_deref(), Some("sqlite-v6"));
    assert!(direct.turn_id.is_some() && direct.context_tables == Some(2));
    assert_eq!(direct.exact_match, None, "só o PostgreSQL tem correspondência exata");

    // Acerto depois da correção: a coluna errada parou no preflight
    let fixed = find(&records, "clientes_por_cidade", "sim-b", "en");
    assert_eq!(fixed.status, "answered");
    assert!(!fixed.correct_first && fixed.correct_final);
    assert_eq!((fixed.attempts.len(), fixed.counted_attempts), (2, 2));
    let wrong = &fixed.attempts[0];
    assert_eq!((wrong.failed_stage, wrong.failure_class), (Some(AssistantStage::Preflight), Some(FailureClass::UnknownObject)));
    assert!(wrong.error.as_deref().is_some_and(|error| error.contains("cidade")), "{:?}", wrong.error);
    assert_eq!((wrong.correct, wrong.row_count), (None, None));
    assert_eq!(fixed.attempts[1].correct, Some(true));
    // As colunas vieram em outra ordem e com outros nomes
    assert_eq!(fixed.comparison.as_ref().unwrap().mapping, [Some(1), Some(0)]);

    // Saída fora do contrato conta como tentativa
    let invalid_output = find(&records, "ticket_medio", "sim-a", "pt");
    assert!(!invalid_output.correct_first && invalid_output.correct_final);
    let first = &invalid_output.attempts[0];
    assert_eq!((first.kind, first.counted), (None, true));
    assert_eq!(first.generation_error.as_deref(), Some("ASSISTANT_INVALID_OUTPUT"));
    assert!(first.raw.as_deref().is_some_and(|raw| raw.starts_with("Claro!")));

    // Consulta inválida até o limite: a escrita para no guard, o resto no banco
    let invalid = find(&records, "faturamento_por_cliente", "sim-b", "pt");
    assert_eq!(invalid.status, "exhausted");
    assert!(!invalid.correct_first && !invalid.correct_final && invalid.result.is_none());
    assert_eq!(invalid.attempts.len(), 4);
    assert_eq!(
        (invalid.attempts[0].failed_stage, invalid.attempts[0].failure_class),
        (Some(AssistantStage::Guard), Some(FailureClass::GuardRejected))
    );
    assert!(invalid.attempts.iter().all(|attempt| attempt.failed_stage.is_some() && attempt.correct.is_none()));
    assert_eq!(bench.orders().await.as_deref(), Some("10"), "nenhuma escrita chegou ao banco");

    // Consulta que roda com o resultado errado
    let wrong_result = find(&records, "maiores_pedidos", "sim-b", "pt");
    assert_eq!(wrong_result.status, "answered");
    assert!(!wrong_result.correct_final);
    let comparison = wrong_result.comparison.as_ref().unwrap();
    assert_eq!((comparison.equal, comparison.mismatch), (false, Some(Mismatch::Values)));
    assert!(find(&records, "maiores_pedidos", "sim-a", "pt").correct_first);

    // Pergunta sem resposta: o acerto é o pedido de esclarecimento
    let clarified = find(&records, "cor_preferida", "sim-a", "en");
    assert!(clarified.unanswerable);
    assert_eq!(clarified.status, "clarification");
    assert!(clarified.correct_first && clarified.correct_final && clarified.result.is_none());
    let guessed = find(&records, "cor_preferida", "sim-b", "en");
    assert_eq!(guessed.status, "answered");
    assert!(!guessed.correct_final, "responder o que não tem resposta é erro");

    // O andamento e as saídas da análise saem do que foi gravado
    let live: Live = serde_json::from_str(&std::fs::read_to_string(bench.path.join("live.json")).unwrap()).unwrap();
    assert_eq!((live.status, live.done, live.total, live.active.len()), (RoundStatus::Finished, 24, 24, 0));
    assert_eq!(live.correct, records.iter().filter(|record| record.correct_final).count());
    assert_eq!(report::results_csv("fluxo", &records).lines().count(), 25);
    let summary = report::summarize(&records, &report::DEFAULT_DIMENSIONS);
    assert_eq!(summary.len(), 4);
    assert_eq!(summary.iter().map(|row| row.tests).sum::<usize>(), 24);
    // O sim-a em português acerta as 6; 4 de primeira
    assert_eq!(summary[1].group, ["sqlite", "sim-a", "pt"]);
    assert_eq!((summary[1].correct_first, summary[1].correct_final), (4, 6));
}

/// Política padrão: ninguém responde, e o turno encerra como esclarecimento.
#[tokio::test]
async fn ask_user_ends_as_clarification_by_default() {
    let bench = Bench::new("ask-clarify", "", &["cor_preferida"]).await;
    assert_eq!(bench.inputs.environment.run.ask_user, AskUser::Clarify);
    let session = bench.session(sim::demo_script());
    bench.round(&session, None).await;

    let records = bench.dir.units().unwrap();
    let asked = find(&records, "cor_preferida", "sim-b", "pt");
    assert_eq!(asked.status, "clarification");
    assert!(asked.correct_final && asked.correct_first);
    assert_eq!(asked.asked_user.len(), 1);
    let question = &asked.asked_user[0];
    assert_eq!((question.policy, question.reply.as_deref()), (AskUser::Clarify, None));
    assert_eq!(question.question, "Em que tabela fica a cor preferida?");
    assert_eq!(question.attempt_id, asked.attempts[0].attempt_id);
    // A pergunta não conta como tentativa, e não houve outra
    assert_eq!((asked.attempts.len(), asked.counted_attempts, asked.attempts[0].counted), (1, 0, false));
    assert_eq!(asked.attempts[0].kind, Some(OutputKind::AskUser));
    assert!(session.finished.lock().contains(&AssistantTurnStatus::Clarification));
}

/// Política `reply`: a resposta fixa, no idioma da pergunta, chega ao modelo.
#[tokio::test]
async fn ask_user_can_be_answered_with_a_fixed_reply() {
    let extra = "ask_user = \"reply\"\nask_user_reply = { pt = \"Use o bom senso.\", en = \"Use your judgment.\" }\n";
    let bench = Bench::new("ask-reply", extra, &["cor_preferida"]).await;
    let session = bench.session(Box::new(|_, _, _| {
        vec![
            Reply::AskUser("Qual coluna?".into()),
            Reply::Query("SELECT name, city FROM customers".into()),
        ]
    }));
    bench.round(&session, None).await;

    let records = bench.dir.units().unwrap();
    for (language, reply) in [("pt", "Use o bom senso."), ("en", "Use your judgment.")] {
        let answered = find(&records, "cor_preferida", "sim-a", language);
        assert_eq!(answered.status, "answered");
        assert!(!answered.correct_final, "a pergunta não tem resposta: responder é erro");
        assert_eq!((answered.asked_user[0].policy, answered.asked_user[0].reply.as_deref()), (AskUser::Reply, Some(reply)));
        assert_eq!((answered.attempts.len(), answered.counted_attempts), (2, 1));
    }
    // A resposta foi junto no pedido da tentativa seguinte, como no app
    let replies: Vec<String> = session.requests.lock().iter().filter_map(|request| request.context.user_reply.clone()).collect();
    assert_eq!(replies.len(), 4);
    assert!(replies.contains(&"Use o bom senso.".to_string()) && replies.contains(&"Use your judgment.".to_string()));
}

#[tokio::test]
async fn an_interrupted_round_resumes_without_repeating() {
    let bench = Bench::new("resume", "", &[]).await;
    let first = bench.session(sim::demo_script());
    let outcome = bench.round(&first, Some(5)).await;
    assert_eq!((outcome.status, outcome.done), (RoundStatus::Interrupted, 5));
    assert_eq!(first.finished.lock().len(), 5);
    let before: Vec<String> = bench.dir.units().unwrap().into_iter().map(|record| record.key).collect();
    // Na ordem do plano
    assert_eq!(before, bench.inputs.units[..5].iter().map(|unit| unit.key()).collect::<Vec<_>>());

    let second = bench.session(sim::demo_script());
    let outcome = bench.round(&second, None).await;
    assert_eq!((outcome.status, outcome.done, outcome.ran), (RoundStatus::Finished, 24, 19));
    assert_eq!(second.finished.lock().len(), 19, "só o que faltava foi à API");
    let after: Vec<String> = bench.dir.units().unwrap().into_iter().map(|record| record.key).collect();
    assert_eq!(after[..5], before[..]);
    assert_eq!(after, bench.inputs.units.iter().map(|unit| unit.key()).collect::<Vec<_>>());
}

/// A validação prévia dos gabaritos, e a leitura como única porta do banco.
#[tokio::test]
async fn gold_must_run_fit_the_page_and_be_deterministic() {
    let bench = Bench::new("gold", "", &[]).await;
    let database = &bench.databases.by_paradigm["sqlite"];

    let with_gold = |query: &str| {
        let source = sim::DEMO_QUESTIONS.replace("SELECT count(*) FROM customers", query);
        super::questions::load(&source).unwrap()
    };
    let problems = |query: &'static str, row_limit: i64| {
        let questions = with_gold(query);
        let databases = &bench.databases;
        async move {
            let only = ["total_clientes".to_string()];
            run_gold(&questions, &only, databases, row_limit)
                .await
                .unwrap_err()
                .iter()
                .map(|problem| problem.to_string())
                .collect::<Vec<_>>()
        }
    };

    let found = problems("SELECT random()", 200).await;
    assert_eq!(found, ["total_clientes: gabarito `sqlite` não é determinístico: duas execuções deram resultados diferentes"]);
    let found = problems("SELECT nope FROM customers", 200).await;
    assert!(found[0].starts_with("total_clientes: gabarito `sqlite` falhou:"), "{found:?}");
    let found = problems("SELECT id FROM orders", 3).await;
    assert!(found[0].contains("passa de 3 linhas"), "{found:?}");

    // Os gabaritos da demonstração passam, um por pergunta com resposta
    assert_eq!(bench.gold.len(), 5);
    let by_city = bench.gold.iter().find(|gold| gold.question_id == "clientes_por_cidade").unwrap();
    assert_eq!((by_city.table.rows.len(), by_city.table.truncated), (3, false));

    // Uma escrita não passa nem chamada direto
    let error = database.read("DELETE FROM orders", 200).await.unwrap_err();
    assert_eq!(error.code(), "GUARD_REJECTED");
    assert_eq!(bench.orders().await.as_deref(), Some("10"));
}

// ── Com a API e os bancos de verdade (só a pedido) ──────────────────────────

/// Os arquivos do e2e, pelas variáveis de ambiente; `None` = nada a fazer.
fn e2e_inputs() -> Option<Inputs> {
    let (Ok(questions), Ok(environment)) = (
        std::env::var("OCTAPUS_EXPERIMENT_E2E_QUESTIONS"),
        std::env::var("OCTAPUS_EXPERIMENT_E2E_ENV"),
    ) else {
        eprintln!("sem OCTAPUS_EXPERIMENT_E2E_QUESTIONS/OCTAPUS_EXPERIMENT_E2E_ENV: nada a fazer");
        return None;
    };
    let inputs = Inputs::new(&super::read_file(&questions).unwrap(), &super::read_file(&environment).unwrap(), Vec::new())
        .unwrap();
    inputs.apply_api_url();
    Some(inputs)
}

/// A validação prévia contra os bancos de teste de verdade: cada gabarito
/// roda duas vezes, só leitura. Não chama a API nem gasta cota.
///
/// ```text
/// OCTAPUS_EXPERIMENT_E2E_QUESTIONS=perguntas.toml OCTAPUS_EXPERIMENT_E2E_ENV=ambiente.toml \
///   cargo test --features experiment --lib experiment::tests::e2e_gold -- --ignored --nocapture
/// ```
#[tokio::test]
#[ignore]
async fn e2e_gold_runs_on_the_real_databases() {
    let Some(inputs) = e2e_inputs() else {
        return;
    };
    let databases = Databases::connect(&inputs.environment, &super::env_var).await.unwrap();
    let (gold, warnings) = run_gold(&inputs.questions, &[], &databases, inputs.environment.run.row_limit)
        .await
        .unwrap_or_else(|problems| panic!("{}", super::problems_text("gabaritos", &problems)));
    println!("{} gabarito(s) conferido(s); avisos: {warnings:?}", gold.len());
    assert!(!gold.is_empty());
}

/// Uma rodada de verdade, pequena: os três primeiros testes do plano, com a
/// API e os bancos do ambiente. **Gasta cota** (três turnos); só a pedido:
///
/// ```text
/// OCTAPUS_EXPERIMENT_E2E_QUESTIONS=perguntas.toml OCTAPUS_EXPERIMENT_E2E_ENV=ambiente.toml \
///   cargo test --features experiment --lib experiment::tests::e2e_real -- --ignored --nocapture
/// ```
#[tokio::test]
#[ignore]
async fn e2e_real_round_with_the_api() {
    let Some(inputs) = e2e_inputs() else {
        return;
    };
    let databases = Databases::connect(&inputs.environment, &super::env_var).await.unwrap();
    let (gold, _) = run_gold(&inputs.questions, &[], &databases, inputs.environment.run.row_limit)
        .await
        .unwrap_or_else(|problems| panic!("{}", super::problems_text("gabaritos", &problems)));
    let (session, _) = super::connect_api(&inputs.environment, false).await.unwrap();

    let path = std::env::temp_dir().join(format!("octapus-experiment-e2e-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    let dir = RunDir::create(&path).unwrap();
    let units = &inputs.units[..inputs.units.len().min(3)];
    let lab = Lab {
        session: &session,
        databases: &databases,
        questions: &inputs.questions,
        gold: &gold,
        run: &inputs.environment.run,
        run_id: "e2e",
    };
    let options = Options {
        concurrency: 1,
        max_consecutive_errors: 1,
        redo_failed: false,
    };
    let outcome = round::run(&lab, units, &dir, "e2e", &options, &CancelFlag::default(), &|record, _, _| {
        println!(
            "{} → {} ({} tentativa(s), primeira {}, final {})",
            record.key,
            record.status,
            record.attempts.len(),
            record.correct_first,
            record.correct_final
        );
    })
    .await
    .unwrap();

    assert_eq!((outcome.status, outcome.ran), (RoundStatus::Finished, units.len()));
    for record in dir.units().unwrap() {
        // Cada teste ficou ligado ao turno e às tentativas da API, na conversa dele
        assert!(record.turn_id.is_some() && record.conversation_id.is_some(), "{}", record.key);
        assert!(!record.attempts.is_empty() && record.attempts.iter().all(|attempt| !attempt.attempt_id.is_empty()));
        assert_eq!(record.model_info.as_ref().map(|model| model.id.as_str()), Some(record.unit.model.as_str()));
    }
    let _ = std::fs::remove_dir_all(path);
}
