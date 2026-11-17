//! Executor de rodadas do experimento do TCC: roda a bateria de perguntas no
//! assistente de verdade, grava cada teste e mostra tudo numa tela local.
//!
//! Ferramenta privada do autor: fica atrás da feature `experiment` e entra
//! como um example do Cargo, então não vai para o app nem para o instalador.
//! O uso está no `README.md` desta pasta; `--help` lista os comandos.
//!
//! ```text
//! perguntas.toml ─┐                       ┌─▶ units.jsonl ─┐
//! ambiente.toml  ─┼─▶ plano ─▶ rodada ────┼─▶ live.json   ─┼─▶ serve (tela)
//!                 │   (plan)   (round)    └─▶ manifest…   ─┘   só lê a pasta
//!                 └─▶ gabaritos (lab) ─▶ cada teste (unit) = `turn::run_turn`
//! ```
//!
//! Regras que valem em todo caminho daqui:
//! - **só leitura**: o que toca um banco passa pelo guard e pelo executor
//!   só-leitura do assistente, inclusive o gabarito;
//! - **o assistente não muda**: o turno é o do app, com as portas embrulhadas
//!   só para anotar;
//! - **rodada real só com confirmação digitada num terminal**: não há opção
//!   que pule isso (quem chama o comando pode ser um script ou uma IA);
//! - **log sem conteúdo**: ids, contagens e códigos. Pergunta e consulta só
//!   nos arquivos da rodada.

mod compare;
mod config;
mod lab;
mod plan;
mod questions;
mod record;
mod report;
mod round;
mod serve;
mod setup;
mod sim;
mod store;
mod unit;

#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use sha2::{Digest, Sha256};

use self::config::Environment;
use self::lab::{ApiSession, Databases, Lab, Session};
use self::plan::{Design, Unit};
use self::questions::{db_type, Problem, QuestionSet};
use self::record::{GoldResult, UnitRecord, CANCELLED, NOT_OPENED};
use self::round::Options;
use self::sim::SimSession;
use self::store::{Manifest, PromptInfo, Resume, RoundStatus, RunDir};
use crate::assistant::{output, prompts, CancelFlag};
use crate::models::{AssistantModel, AssistantModelSelection};

const USAGE: &str = "\
Executor de rodadas do experimento (TCC)

  check   --questions ARQ [--env ARQ]     confere as perguntas; com --env, executa cada
                                          gabarito duas vezes (não chama a API)
  run     --questions ARQ --env ARQ       roda a bateria (pede confirmação no terminal)
          [--out PASTA] [--only id,id]    --only restringe a algumas perguntas
          [--dry-run]                     --dry-run só informa quantos testes rodariam
          [--accept-consent]              aceita o termo vigente da API, se faltar
  resume  PASTA_DA_RODADA                 continua de onde parou, sem repetir
          [--redo-failed] [--dry-run]     --redo-failed refaz os turnos que falharam
          [--allow-version-change]        aceita app ou prompt diferente do início
  report  PASTA_DA_RODADA                 regrava results.csv e summary.csv
  serve   [--runs PASTA] [--port N]       abre a tela (só em 127.0.0.1)
  demo    [--out PASTA] [--delay-ms N]    rodada simulada, sem rede nem cota
          [--concurrency N]

  --verbose   mostra o log (ids, contagens e códigos) no terminal

Pasta padrão das rodadas: experiment-runs/";

const DEFAULT_RUNS: &str = "experiment-runs";
const DEFAULT_PORT: u16 = 7878;

/// A entrada do example `experiment`.
pub fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let code = match Cli::parse(&args) {
        Ok(cli) => {
            if cli.verbose {
                install_logger();
            }
            match execute(cli) {
                Ok(()) => 0,
                Err(message) => {
                    eprintln!("erro: {message}");
                    1
                }
            }
        }
        Err(message) => {
            eprintln!("{message}\n\n{USAGE}");
            2
        }
    };
    std::process::exit(code);
}

#[derive(Debug, Default, PartialEq, Eq)]
struct Cli {
    command: String,
    /// O argumento sem nome (a pasta da rodada)
    target: Option<String>,
    options: BTreeMap<String, String>,
    flags: Vec<String>,
    verbose: bool,
}

impl Cli {
    const VALUED: [&'static str; 8] =
        ["--questions", "--env", "--out", "--only", "--runs", "--port", "--delay-ms", "--concurrency"];
    const FLAGS: [&'static str; 4] = ["--dry-run", "--accept-consent", "--redo-failed", "--allow-version-change"];

    fn parse(args: &[String]) -> Result<Self, String> {
        let mut cli = Self::default();
        let mut args = args.iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--help" | "-h" | "help" => return Err(String::new()),
                "--verbose" => cli.verbose = true,
                name if Self::VALUED.contains(&name) => {
                    let value = args.next().ok_or_else(|| format!("{name} precisa de um valor"))?;
                    cli.options.insert(name.to_string(), value.clone());
                }
                name if Self::FLAGS.contains(&name) => cli.flags.push(name.to_string()),
                name if name.starts_with('-') => return Err(format!("opção desconhecida: {name}")),
                value if cli.command.is_empty() => cli.command = value.to_string(),
                value if cli.target.is_none() => cli.target = Some(value.to_string()),
                value => return Err(format!("argumento a mais: {value}")),
            }
        }
        if cli.command.is_empty() {
            return Err(String::new());
        }
        Ok(cli)
    }

    fn option(&self, name: &str) -> Option<&str> {
        self.options.get(name).map(String::as_str)
    }

    fn required(&self, name: &str) -> Result<&str, String> {
        self.option(name).ok_or_else(|| format!("falta {name}"))
    }

    fn flag(&self, name: &str) -> bool {
        self.flags.iter().any(|flag| flag == name)
    }

    fn number<T: std::str::FromStr>(&self, name: &str, default: T) -> Result<T, String> {
        match self.option(name) {
            Some(value) => value.parse().map_err(|_| format!("{name}: número inválido")),
            None => Ok(default),
        }
    }

    fn target(&self) -> Result<&Path, String> {
        self.target.as_deref().map(Path::new).ok_or_else(|| "falta a pasta da rodada".to_string())
    }
}

fn execute(cli: Cli) -> Result<(), String> {
    match cli.command.as_str() {
        "check" => check(&cli),
        "run" => run(&cli),
        "resume" => resume(&cli),
        "report" => {
            let dir = RunDir::open(cli.target()?).map_err(text)?;
            let manifest = dir.manifest().map_err(text)?;
            write_reports(&dir, &manifest.run_id)?;
            println!("results.csv e summary.csv regravados em {}", dir.path().display());
            Ok(())
        }
        "serve" => {
            let root = PathBuf::from(cli.option("--runs").unwrap_or(DEFAULT_RUNS));
            let port = cli.number("--port", DEFAULT_PORT)?;
            runtime()?.block_on(serve::serve(root, port)).map_err(text)
        }
        "demo" => demo(&cli),
        other => Err(format!("comando desconhecido: {other} (veja --help)")),
    }
}

fn text(error: impl std::fmt::Display) -> String {
    error.to_string()
}

fn runtime() -> Result<tokio::runtime::Runtime, String> {
    tokio::runtime::Builder::new_multi_thread().enable_all().build().map_err(text)
}

fn env_var(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

fn read_file(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))
}

fn load_questions(source: &str) -> Result<QuestionSet, String> {
    questions::load(source).map_err(|problems| problems_text("o conjunto de perguntas tem problemas", &problems))
}

fn problems_text(title: &str, problems: &[Problem]) -> String {
    let lines: Vec<String> = problems.iter().map(|problem| format!("  - {problem}")).collect();
    format!("{title}:\n{}", lines.join("\n"))
}

fn only(cli: &Cli) -> Vec<String> {
    cli.option("--only")
        .map(|list| list.split(',').map(|id| id.trim().to_string()).filter(|id| !id.is_empty()).collect())
        .unwrap_or_default()
}

/// `check`: a validação prévia. Não chama a API.
fn check(cli: &Cli) -> Result<(), String> {
    let questions = load_questions(&read_file(cli.required("--questions")?)?)?;
    let with_gold = questions.questions.iter().filter(|question| !question.unanswerable).count();
    println!(
        "{} pergunta(s): {with_gold} com gabarito, {} sem resposta possível. Formato, idiomas e leitura conferidos.",
        questions.questions.len(),
        questions.questions.len() - with_gold
    );
    let Some(path) = cli.option("--env") else {
        println!("Sem --env: os gabaritos não foram executados.");
        return Ok(());
    };

    let environment = config::load(&read_file(path)?)?;
    runtime()?.block_on(async {
        let databases = Databases::connect(&environment, &env_var).await?;
        let (gold, warnings) = lab::run_gold(&questions, &[], &databases, environment.run.row_limit)
            .await
            .map_err(|problems| problems_text("os gabaritos têm problemas", &problems))?;
        for warning in &warnings {
            println!("aviso: {warning}");
        }
        println!("{} gabarito(s) executado(s) duas vezes, com o mesmo resultado.", gold.len());
        Ok(())
    })
}

/// O que uma rodada precisa, lido dos arquivos (ou do manifesto, na retomada).
struct Inputs {
    questions: QuestionSet,
    environment: Environment,
    environment_toml: String,
    only: Vec<String>,
    units: Vec<Unit>,
}

impl Inputs {
    fn new(questions_toml: &str, environment_toml: &str, only: Vec<String>) -> Result<Self, String> {
        let questions = load_questions(questions_toml)?;
        let environment = config::load(environment_toml)?;
        for id in &only {
            if questions.get(id).is_none() {
                return Err(format!("--only: a pergunta `{id}` não existe"));
            }
        }
        let units = plan::units(&Design {
            questions: &questions,
            only: &only,
            paradigms: &environment.paradigms(),
            languages: &environment.run.languages,
            models: &environment.api.models,
            repetitions: environment.run.repetitions,
            seed: environment.run.seed,
        });
        if units.is_empty() {
            return Err("nenhum teste a rodar: as perguntas não têm gabarito para os bancos do ambiente".into());
        }
        Ok(Self {
            questions,
            environment,
            environment_toml: environment_toml.to_string(),
            only,
            units,
        })
    }

    /// Quantos testes, por paradigma, modelo e idioma.
    fn describe(&self, units: &[&Unit]) -> String {
        let mut count: BTreeMap<(String, String, &str), usize> = BTreeMap::new();
        for unit in units {
            *count.entry((unit.paradigm.clone(), unit.model.clone(), unit.language.code())).or_default() += 1;
        }
        let mut lines = vec![format!(
            "{} teste(s): {} repetição(ões), semente {}, {} por vez",
            units.len(),
            self.environment.run.repetitions,
            self.environment.run.seed,
            self.environment.run.concurrency
        )];
        for ((paradigm, model, language), tests) in count {
            lines.push(format!("  {paradigm:<10} {model:<28} {language}  {tests}"));
        }
        lines.join("\n")
    }

    /// A API vale a do arquivo, quando ele diz uma (antes de qualquer tarefa
    /// começar: a variável é lida na partida do cliente, como no app).
    fn apply_api_url(&self) {
        if let Some(url) = &self.environment.api.url {
            std::env::set_var(config::API_URL_ENV, url);
        }
    }
}

/// `run`: uma rodada nova.
fn run(cli: &Cli) -> Result<(), String> {
    let inputs = Inputs::new(
        &read_file(cli.required("--questions")?)?,
        &read_file(cli.required("--env")?)?,
        only(cli),
    )?;
    println!("{}", inputs.describe(&inputs.units.iter().collect::<Vec<_>>()));
    if cli.flag("--dry-run") {
        println!("--dry-run: nada foi executado.");
        return Ok(());
    }
    inputs.apply_api_url();

    let out = PathBuf::from(cli.option("--out").unwrap_or(DEFAULT_RUNS));
    let run_id = format!("{}-s{}", chrono::Local::now().format("%Y%m%d-%H%M%S"), inputs.environment.run.seed);
    runtime()?.block_on(async {
        let (databases, gold) = prepare_databases(&inputs).await?;
        let (session, models) = connect_api(&inputs.environment, cli.flag("--accept-consent")).await?;
        confirm(inputs.units.len(), session.url())?;

        let dir = RunDir::create(&out.join(&run_id)).map_err(text)?;
        let manifest = manifest(&run_id, &inputs, models, session.url().map(str::to_string), false);
        dir.write_manifest(&manifest).map_err(text)?;
        dir.write_gold(&gold).map_err(text)?;
        drive(&dir, &manifest, &inputs, &session, &databases, &gold, false).await
    })
}

/// `resume`: continua uma rodada com o que ela guardou no começo.
fn resume(cli: &Cli) -> Result<(), String> {
    let dir = RunDir::open(cli.target()?).map_err(text)?;
    let mut manifest = dir.manifest().map_err(text)?;
    let inputs = Inputs::new(&manifest.questions_toml, &manifest.environment_toml, manifest.only.clone())?;
    if inputs.units.iter().map(Unit::key).collect::<Vec<_>>() != manifest.order {
        return Err("o plano desta versão não é o que a rodada gravou: retome com a versão que a iniciou".into());
    }

    let redo_failed = cli.flag("--redo-failed");
    let recorded = dir.units().map_err(text)?;
    let pending = round::pending(&inputs.units, &recorded, redo_failed);
    println!("Rodada {}: {} de {} feitos.", manifest.run_id, inputs.units.len() - pending.len(), inputs.units.len());
    if pending.is_empty() {
        println!("Nada a retomar.");
        return Ok(());
    }
    println!("Faltam:\n{}", inputs.describe(&pending));
    if cli.flag("--dry-run") {
        println!("--dry-run: nada foi executado.");
        return Ok(());
    }

    // O que mede precisa ser o que media no começo
    let changed = version_changes(&manifest, &inputs);
    if !changed.is_empty() && !cli.flag("--allow-version-change") {
        return Err(format!(
            "mudou desde o início da rodada: {}. Retome com a mesma versão, ou aceite com --allow-version-change (fica registrado)",
            changed.join("; ")
        ));
    }
    inputs.apply_api_url();

    runtime()?.block_on(async {
        let (databases, gold) = prepare_databases(&inputs).await?;
        if gold != dir.gold().map_err(text)? {
            return Err("o resultado dos gabaritos mudou: o banco não é o do início da rodada".to_string());
        }
        let (commit, _) = git_commit();
        manifest.resumes.push(Resume {
            at: record::now(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            commit,
        });

        if manifest.simulated {
            let session = demo_session(&inputs.questions, Duration::from_millis(400));
            dir.write_manifest(&manifest).map_err(text)?;
            return drive(&dir, &manifest, &inputs, &session, &databases, &gold, redo_failed).await;
        }
        let (session, _) = connect_api(&inputs.environment, false).await?;
        confirm(pending.len(), session.url())?;
        dir.write_manifest(&manifest).map_err(text)?;
        drive(&dir, &manifest, &inputs, &session, &databases, &gold, redo_failed).await
    })
}

/// `demo`: uma rodada inteira com a API simulada, num SQLite descartável.
fn demo(cli: &Cli) -> Result<(), String> {
    let out = PathBuf::from(cli.option("--out").unwrap_or(DEFAULT_RUNS));
    let delay = Duration::from_millis(cli.number("--delay-ms", 400)?);
    let concurrency: usize = cli.number("--concurrency", 1)?;
    let run_id = format!("{}-demo", chrono::Local::now().format("%Y%m%d-%H%M%S"));
    let dir = RunDir::create(&out.join(&run_id)).map_err(text)?;

    let database = std::path::absolute(dir.path().join("demo.sqlite")).map_err(text)?;
    sim::create_demo_database(&database)?;
    let inputs = Inputs::new(sim::DEMO_QUESTIONS, &sim::demo_environment(&database, concurrency.max(1)), Vec::new())?;
    println!("Rodada simulada (sem rede).\n{}", inputs.describe(&inputs.units.iter().collect::<Vec<_>>()));

    runtime()?.block_on(async {
        let (databases, gold) = prepare_databases(&inputs).await?;
        let session = demo_session(&inputs.questions, delay);
        let models = sim::DEMO_MODELS.iter().map(|id| sim::model(id)).collect();
        let manifest = manifest(&run_id, &inputs, models, None, true);
        dir.write_manifest(&manifest).map_err(text)?;
        dir.write_gold(&gold).map_err(text)?;
        drive(&dir, &manifest, &inputs, &session, &databases, &gold, false).await
    })
}

fn demo_session(questions: &QuestionSet, delay: Duration) -> SimSession {
    SimSession::new(questions, sim::demo_script(), delay)
}

/// Conecta nos bancos e executa os gabaritos (a validação prévia inteira).
async fn prepare_databases(inputs: &Inputs) -> Result<(Databases, Vec<GoldResult>), String> {
    let databases = Databases::connect(&inputs.environment, &env_var).await?;
    let (gold, warnings) = lab::run_gold(&inputs.questions, &inputs.only, &databases, inputs.environment.run.row_limit)
        .await
        .map_err(|problems| problems_text("os gabaritos têm problemas", &problems))?;
    for warning in &warnings {
        println!("aviso: {warning}");
    }
    println!("{} gabarito(s) conferido(s).", gold.len());
    Ok((databases, gold))
}

/// Liga na API e confere o que a rodada precisa dela: termo aceito, modelo
/// de livre escolha e os modelos do ambiente. Nada disto gera tentativa.
async fn connect_api(environment: &Environment, accept_consent: bool) -> Result<(ApiSession, Vec<AssistantModel>), String> {
    let api = |error: crate::error::Error| format!("API do assistente: {error}");
    let session = ApiSession::connect(&environment.api, &env_var).await.map_err(api)?;
    let me = session.me().await.map_err(api)?;

    if let Some(current) = me.consent.current_version.as_deref() {
        if me.consent.accepted_version.as_deref() != Some(current) {
            if !accept_consent {
                return Err("o termo vigente da API não foi aceito por este participante: aceite no app ou rode com --accept-consent".into());
            }
            session.accept_consent(current).await.map_err(api)?;
        }
    }
    if me.model_selection != AssistantModelSelection::Free {
        return Err("o participante não está no modo de escolha livre de modelo: a rodada não consegue fixar o modelo de cada teste".into());
    }
    let available = session.models().await.map_err(api)?;
    let mut models = Vec::new();
    for id in &environment.api.models {
        match available.iter().find(|model| &model.id == id) {
            Some(model) => models.push(model.clone()),
            None => {
                let known: Vec<&str> = available.iter().map(|model| model.id.as_str()).collect();
                return Err(format!("o modelo `{id}` não existe na API. Disponíveis: {}", known.join(", ")));
            }
        }
    }
    println!(
        "API {}: participante {}, cota {} de {} tokens usados.",
        session.url().unwrap_or("?"),
        me.participant.label,
        me.quota.used_tokens,
        me.quota.limit_tokens
    );
    Ok((session, models))
}

/// A confirmação de uma rodada real: digitada, num terminal.
fn confirm(tests: usize, url: Option<&str>) -> Result<(), String> {
    if !io::stdin().is_terminal() {
        return Err("uma rodada real só começa com confirmação digitada num terminal".into());
    }
    print!(
        "\nRodada REAL: {tests} teste(s) na API {} (gasta cota).\nDigite {tests} para começar: ",
        url.unwrap_or("?")
    );
    io::stdout().flush().map_err(text)?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer).map_err(text)?;
    match answer.trim() == tests.to_string() {
        true => Ok(()),
        false => Err("não confirmado: nada foi executado".into()),
    }
}

/// O prompt de cada paradigma da rodada, com o hash que a API confere.
fn prompt_infos(paradigms: &[String]) -> Vec<PromptInfo> {
    paradigms
        .iter()
        .filter_map(|paradigm| {
            let prompt = prompts::for_db(db_type(paradigm)?)?;
            Some(PromptInfo {
                paradigm: paradigm.clone(),
                version: prompt.version.to_string(),
                sha256: hex::encode(Sha256::digest(prompt.text.as_bytes())),
            })
        })
        .collect()
}

/// O commit do código (o example roda de dentro do repositório) e se havia
/// mudança fora dele.
fn git_commit() -> (Option<String>, Option<bool>) {
    let git = |args: &[&str]| {
        Command::new("git")
            .args(["-C", env!("CARGO_MANIFEST_DIR")])
            .args(args)
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string())
    };
    let commit = git(&["rev-parse", "HEAD"]);
    let dirty = git(&["status", "--porcelain"]).map(|status| !status.is_empty());
    (commit, dirty)
}

fn manifest(run_id: &str, inputs: &Inputs, models: Vec<AssistantModel>, api_url: Option<String>, simulated: bool) -> Manifest {
    let (commit, commit_dirty) = git_commit();
    let run = &inputs.environment.run;
    let paradigms = inputs.environment.paradigms();
    Manifest {
        run_id: run_id.to_string(),
        created_at: record::now(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        commit,
        commit_dirty,
        contract_version: output::CONTRACT_VERSION.to_string(),
        api_url,
        backend: inputs.environment.api.backend,
        simulated,
        prompts: prompt_infos(&paradigms),
        models,
        paradigms,
        languages: run.languages.clone(),
        repetitions: run.repetitions,
        seed: run.seed,
        concurrency: run.concurrency,
        ask_user: run.ask_user,
        ask_user_reply: run.ask_user_reply.clone(),
        mongo_sample_values: run.mongo_sample_values,
        row_limit: run.row_limit,
        only: inputs.only.clone(),
        questions_version: inputs.questions.version.clone(),
        questions_sha256: inputs.questions.sha256.clone(),
        questions_toml: inputs.questions.source.clone(),
        environment_toml: inputs.environment_toml.clone(),
        total: inputs.units.len(),
        order: inputs.units.iter().map(Unit::key).collect(),
        resumes: Vec::new(),
    }
}

/// O que não é mais o do começo da rodada (versão do app e prompts).
fn version_changes(manifest: &Manifest, inputs: &Inputs) -> Vec<String> {
    let mut changes = Vec::new();
    if manifest.app_version != env!("CARGO_PKG_VERSION") {
        changes.push(format!("versão do app ({} → {})", manifest.app_version, env!("CARGO_PKG_VERSION")));
    }
    let (commit, _) = git_commit();
    if manifest.commit != commit {
        changes.push("commit".into());
    }
    if manifest.prompts != prompt_infos(&inputs.environment.paradigms()) {
        changes.push("prompt".into());
    }
    changes
}

/// Roda o que falta, com a interrupção pelo Ctrl+C, e fecha com os relatórios.
async fn drive(
    dir: &RunDir,
    manifest: &Manifest,
    inputs: &Inputs,
    session: &dyn Session,
    databases: &Databases,
    gold: &[GoldResult],
    redo_failed: bool,
) -> Result<(), String> {
    let stop = CancelFlag::default();
    let interrupt = {
        let stop = stop.clone();
        tokio::spawn(async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                eprintln!("\nInterrompendo: parando os testes em andamento (Ctrl+C de novo encerra na hora)…");
                stop.cancel();
                if tokio::signal::ctrl_c().await.is_ok() {
                    std::process::exit(130);
                }
            }
        })
    };

    println!(
        "\nRodada {} em {}\nTela: cargo run --example experiment --features experiment -- serve --runs {}\n",
        manifest.run_id,
        dir.path().display(),
        dir.path().parent().unwrap_or(Path::new(".")).display()
    );
    let lab = Lab {
        session,
        databases,
        questions: &inputs.questions,
        gold,
        run: &inputs.environment.run,
        run_id: &manifest.run_id,
    };
    let options = Options {
        concurrency: inputs.environment.run.concurrency,
        max_consecutive_errors: inputs.environment.run.max_consecutive_errors,
        redo_failed,
    };
    let outcome = round::run(&lab, &inputs.units, dir, &manifest.run_id, &options, &stop, &progress_line)
        .await
        .map_err(text)?;
    interrupt.abort();

    write_reports(dir, &manifest.run_id)?;
    println!("\n{}", summary_text(&dir.units().map_err(text)?));
    let resume_hint = format!(
        "Para continuar: cargo run --example experiment --features experiment -- resume {}",
        dir.path().display()
    );
    match outcome.status {
        RoundStatus::Finished => println!("Rodada completa: {} de {} testes.", outcome.done, outcome.total),
        RoundStatus::Interrupted => {
            println!("Rodada interrompida: {} de {} testes gravados.\n{resume_hint}", outcome.done, outcome.total)
        }
        RoundStatus::StoppedOnErrors | RoundStatus::Running => println!(
            "Rodada parada: testes seguidos não abriram ({}). {} de {} gravados.\n{resume_hint}",
            outcome.last_error.as_deref().unwrap_or("?"),
            outcome.done,
            outcome.total
        ),
    }
    Ok(())
}

/// A linha de cada teste terminado: só ids, estado e contagens.
fn progress_line(record: &UnitRecord, done: usize, total: usize) {
    let mark = match (record.status.as_str(), record.correct_final) {
        (NOT_OPENED, _) => "!",
        (CANCELLED, _) => "~",
        (_, true) => "✓",
        (_, false) => "✗",
    };
    println!(
        "[{done:>4}/{total}] {mark} {:<24} {:<9} {} {:<24} r{}  {:<14} {} tent.  {:.1}s{}",
        record.unit.question_id,
        record.unit.paradigm,
        record.unit.language.code(),
        record.unit.model,
        record.unit.repetition,
        record.status,
        record.attempts.len(),
        record.turn_ms as f64 / 1000.0,
        record.error_code.as_deref().map(|code| format!("  {code}")).unwrap_or_default()
    );
}

fn write_reports(dir: &RunDir, run_id: &str) -> Result<(), String> {
    let records = dir.units().map_err(text)?;
    let summary = report::summarize(&records, &report::DEFAULT_DIMENSIONS);
    dir.write_text(store::RESULTS_CSV, &report::results_csv(run_id, &records)).map_err(text)?;
    dir.write_text(store::SUMMARY_CSV, &report::summary_csv(&summary, &report::DEFAULT_DIMENSIONS))
        .map_err(text)
}

fn summary_text(records: &[UnitRecord]) -> String {
    let mut lines = vec![format!(
        "{:<10} {:<28} {:<3} {:>6} {:>9} {:>9} {:>10}",
        "paradigma", "modelo", "id.", "testes", "1ª tent.", "final", "inválidas"
    )];
    for row in report::summarize(records, &report::DEFAULT_DIMENSIONS) {
        lines.push(format!(
            "{:<10} {:<28} {:<3} {:>6} {:>8.1}% {:>8.1}% {:>10}",
            row.group[0],
            row.group[1],
            row.group[2],
            row.tests,
            row.accuracy_first * 100.0,
            row.accuracy_final * 100.0,
            row.invalid_query_rate.map(|rate| format!("{:.1}%", rate * 100.0)).unwrap_or_else(|| "-".into())
        ));
    }
    lines.join("\n")
}

/// `--verbose`: o log do projeto no terminal (lá só vão ids, contagens e
/// códigos; o app usa o plugin de log do Tauri, que aqui não existe).
fn install_logger() {
    struct Stderr;
    impl log::Log for Stderr {
        fn enabled(&self, metadata: &log::Metadata) -> bool {
            metadata.level() <= log::Level::Info
        }

        fn log(&self, record: &log::Record) {
            if self.enabled(record.metadata()) && record.target().starts_with("octapus_db_lib") {
                eprintln!("[{}] {}", record.level(), record.args());
            }
        }

        fn flush(&self) {}
    }
    static LOGGER: Stderr = Stderr;
    if log::set_logger(&LOGGER).is_ok() {
        log::set_max_level(log::LevelFilter::Info);
    }
}

#[cfg(test)]
mod cli_tests {
    use super::*;

    fn parse(line: &str) -> Result<Cli, String> {
        Cli::parse(&line.split_whitespace().map(String::from).collect::<Vec<_>>())
    }

    #[test]
    fn parses_commands_options_and_flags() {
        let cli = parse("run --questions q.toml --env e.toml --only a,b --dry-run").unwrap();
        assert_eq!(cli.command, "run");
        assert_eq!(cli.required("--questions"), Ok("q.toml"));
        assert_eq!(only(&cli), ["a", "b"]);
        assert!(cli.flag("--dry-run") && !cli.flag("--redo-failed"));

        let cli = parse("resume runs/x --redo-failed --verbose").unwrap();
        assert_eq!((cli.target().unwrap(), cli.verbose), (Path::new("runs/x"), true));
        assert_eq!(parse("serve --port 9000").unwrap().number("--port", DEFAULT_PORT), Ok(9000));

        assert!(parse("run --questions").unwrap_err().contains("precisa de um valor"));
        assert!(parse("run --yes").unwrap_err().contains("opção desconhecida"));
        assert_eq!(parse("--help").unwrap_err(), "");
        assert!(parse("run").unwrap().required("--env").is_err());
    }

    /// Não existe opção que confirme uma rodada real: só o terminal.
    #[test]
    fn a_real_round_is_never_confirmed_without_a_terminal() {
        for bypass in ["--yes", "--confirm", "--force", "-y"] {
            assert!(parse(&format!("run {bypass}")).is_err(), "{bypass}");
        }
        // Os testes rodam sem terminal na entrada
        if !io::stdin().is_terminal() {
            assert!(confirm(3, None).unwrap_err().contains("terminal"));
        }
    }
}
