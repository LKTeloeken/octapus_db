//! O controle de uma rodada: roda os testes que faltam na ordem do plano, com
//! a concorrência pedida, grava cada um ao terminar e mantém o `live.json`.
//!
//! - **Retomar** é rodar de novo sobre a mesma pasta: o que já está no
//!   `units.jsonl` é pulado.
//! - **Interromper** (o [`CancelFlag`] da rodada) para os turnos em andamento
//!   pelo cancelamento do próprio assistente e não começa outros. Um teste
//!   interrompido não vale como resultado: volta na retomada.
//! - **Erros seguidos** (teste que nem abriu ou turno que falhou no meio: API
//!   fora, provedor fora, crédito no fim) pausam a rodada por alguns minutos
//!   e ela tenta de novo; depois de [`Options::max_pauses`] pausas, para. Um
//!   erro fatal ([`FATAL_CODES`]: token, cota, modelo, prompt) para na hora.
//! - **Taxa de falhas** acima do teto, mesmo sem serem seguidas, para.
//! - **Teto de custo** de um modelo, pelo custo que a API mede, para.
//!
//! Parada sozinha deixa o motivo no `live.json` e tudo que foi feito gravado:
//! `resume` continua (com `--redo-failed`, refaz os turnos que falharam).

use std::collections::{BTreeMap, HashSet};
use std::io;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use futures_util::StreamExt;
use parking_lot::Mutex;
use tokio::sync::Notify;

use super::plan::Unit;
use super::record::{now, LiveUnit, UnitRecord, FAILED, NOT_OPENED};
use super::store::{Live, RoundStatus, RunDir};
use super::unit::Observer;
use crate::assistant::CancelFlag;

/// O `live.json` é regravado ao menos neste intervalo, mesmo sem novidade: é
/// o sinal de que o processo da rodada está vivo.
const HEARTBEAT: Duration = Duration::from_secs(2);
/// Novidades em sequência viram uma gravação só.
const COALESCE: Duration = Duration::from_millis(150);

/// Erros que tentar de novo não resolve: a rodada para na hora. O pedido
/// inválido é um deles: a API recusa o formato (versão do app e da API que não
/// combinam), e a mesma recusa viria em todo teste.
pub const FATAL_CODES: [&str; 9] = [
    "ASSISTANT_INVALID_REQUEST",
    "ASSISTANT_UNAUTHORIZED",
    "ASSISTANT_QUOTA_EXCEEDED",
    "ASSISTANT_CONSENT_REQUIRED",
    "ASSISTANT_MODEL_SELECTION_FORBIDDEN",
    "ASSISTANT_MODEL_UNAVAILABLE",
    "ASSISTANT_PROMPT_NOT_REGISTERED",
    "ASSISTANT_NOT_CONFIGURED",
    "EXPERIMENT_MODEL_NOT_FIXED",
];

/// Quem roda um teste: o laboratório de verdade (`lab::Lab`) ou um simulado.
#[async_trait]
pub trait Runner: Send + Sync {
    async fn run(&self, unit: &Unit, cancel: &CancelFlag, observe: Observer<'_>) -> UnitRecord;
}

pub struct Options {
    pub concurrency: usize,
    /// Erros seguidos que pausam a rodada
    pub max_consecutive_errors: u32,
    /// A pausa depois deles; zero = para direto
    pub pause: Duration,
    pub max_pauses: u32,
    /// Fração de `failed` entre os testes desta execução que para a rodada,
    /// contada depois de `failure_rate_after` testes
    pub max_failure_rate: f64,
    pub failure_rate_after: usize,
    /// Teto de gasto por modelo, em dólares
    pub max_cost_usd: BTreeMap<String, f64>,
    /// Roda de novo os testes cujo turno terminou em `failed`
    pub redo_failed: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            concurrency: 1,
            max_consecutive_errors: 3,
            pause: Duration::from_secs(10 * 60),
            max_pauses: 3,
            max_failure_rate: 0.10,
            failure_rate_after: 30,
            max_cost_usd: BTreeMap::new(),
            redo_failed: false,
        }
    }
}

/// Como a rodada (esta execução dela) terminou.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub status: RoundStatus,
    pub total: usize,
    /// Concluídos, contando os de execuções anteriores
    pub done: usize,
    pub correct: usize,
    /// Rodados e gravados nesta execução
    pub ran: usize,
    pub last_error: Option<String>,
    /// Por que parou sozinha
    pub stop_reason: Option<String>,
}

struct State {
    live: Live,
    active: BTreeMap<String, LiveUnit>,
    consecutive_errors: u32,
    ran: usize,
    /// Dos rodados nesta execução, os que terminaram em `failed`
    failed: usize,
    /// Gasto de cada modelo, contando as execuções anteriores
    spend: BTreeMap<String, f64>,
    paused_until: Option<Instant>,
    /// Parou sozinha: com que estado e por quê
    stopped: Option<(RoundStatus, String)>,
    failure: Option<io::Error>,
}

/// O que um teste custou, pelo consumo que a API mediu em cada tentativa.
pub fn cost(record: &UnitRecord) -> f64 {
    record.attempts.iter().filter_map(|attempt| attempt.usage.as_ref()).map(|usage| usage.cost_usd).sum()
}

impl State {
    fn stop_with(&mut self, status: RoundStatus, reason: String, stop: &CancelFlag) {
        if self.stopped.is_none() {
            log::warn!("experimento: rodada parou sozinha ({status:?})");
            self.stopped = Some((status, reason));
        }
        stop.cancel();
    }

    /// Os controles de erro e de custo, com o teste que acabou de terminar.
    fn check(&mut self, record: &UnitRecord, options: &Options, stop: &CancelFlag) {
        let model = &record.unit.model;
        let spent = self.spend.entry(model.clone()).or_default();
        *spent += cost(record);
        let spent = *spent;
        if let Some(limit) = options.max_cost_usd.get(model).filter(|limit| spent >= **limit) {
            let reason = format!("o modelo {model} chegou ao teto de custo (US$ {spent:.2} de US$ {limit:.2})");
            self.stop_with(RoundStatus::StoppedOnBudget, reason, stop);
            return;
        }

        let errored = record.status == NOT_OPENED || record.status == FAILED;
        if !errored {
            if record.is_result() {
                self.consecutive_errors = 0;
            }
            return;
        }
        let code = record.error_code.clone().unwrap_or_else(|| "?".into());
        self.live.last_error = Some(code.clone());
        if FATAL_CODES.contains(&code.as_str()) {
            self.stop_with(RoundStatus::StoppedOnErrors, format!("erro que tentar de novo não resolve ({code})"), stop);
            return;
        }
        if record.status == FAILED {
            self.failed += 1;
            let rate = self.failed as f64 / self.ran.max(1) as f64;
            if self.ran >= options.failure_rate_after && rate > options.max_failure_rate {
                let reason = format!(
                    "{} de {} testes desta execução falharam ({:.0}%, o teto é {:.0}%); último erro: {code}",
                    self.failed,
                    self.ran,
                    rate * 100.0,
                    options.max_failure_rate * 100.0
                );
                self.stop_with(RoundStatus::StoppedOnErrors, reason, stop);
                return;
            }
        }
        // Com vários testes ao mesmo tempo, os que já estavam no ar quando a
        // pausa começou não valem por uma pausa nova
        if self.paused_until.is_some_and(|until| until > Instant::now()) {
            return;
        }
        self.consecutive_errors += 1;
        if self.consecutive_errors < options.max_consecutive_errors {
            return;
        }
        self.consecutive_errors = 0;
        if self.live.pauses < options.max_pauses && !options.pause.is_zero() {
            self.live.pauses += 1;
            self.paused_until = Some(Instant::now() + options.pause);
            log::warn!("experimento: {} erros seguidos, pausa {} de {}", options.max_consecutive_errors, self.live.pauses, options.max_pauses);
            return;
        }
        let reason = format!(
            "{} erros seguidos depois de {} pausa(s); último erro: {code}",
            options.max_consecutive_errors, self.live.pauses
        );
        self.stop_with(RoundStatus::StoppedOnErrors, reason, stop);
    }
}

/// Os testes do plano que ainda faltam, dado o que a pasta já tem.
pub fn pending<'a>(units: &'a [Unit], recorded: &[UnitRecord], redo_failed: bool) -> Vec<&'a Unit> {
    let done: HashSet<&str> = recorded
        .iter()
        .filter(|record| !(redo_failed && record.status == FAILED))
        .map(|record| record.key.as_str())
        .collect();
    units.iter().filter(|unit| !done.contains(unit.key().as_str())).collect()
}

/// Roda o que falta. `report` recebe cada teste terminado, com quantos já
/// foram e o total (a linha de progresso do terminal).
pub async fn run(
    runner: &dyn Runner,
    units: &[Unit],
    dir: &RunDir,
    run_id: &str,
    options: &Options,
    stop: &CancelFlag,
    report: &(dyn Fn(&UnitRecord, usize, usize) + Send + Sync),
) -> io::Result<Outcome> {
    let recorded = dir.units()?;
    let pending = pending(units, &recorded, options.redo_failed);
    let planned: HashSet<String> = units.iter().map(Unit::key).collect();
    let waiting: HashSet<String> = pending.iter().map(|unit| unit.key()).collect();
    let finished: Vec<&UnitRecord> = recorded
        .iter()
        .filter(|record| planned.contains(&record.key) && !waiting.contains(&record.key))
        .collect();
    let correct = finished.iter().filter(|record| record.correct_final).count();

    let started = Instant::now();
    let state = Mutex::new(State {
        live: Live {
            run_id: run_id.to_string(),
            status: RoundStatus::Running,
            pid: std::process::id(),
            started_at: now(),
            updated_at: now(),
            total: units.len(),
            done: finished.len(),
            correct,
            incorrect: finished.len() - correct,
            not_opened: 0,
            last_error: None,
            eta_seconds: None,
            active: Vec::new(),
            pauses: 0,
            paused_until: None,
            stop_reason: None,
        },
        active: BTreeMap::new(),
        consecutive_errors: 0,
        ran: 0,
        failed: 0,
        spend: recorded.iter().fold(BTreeMap::new(), |mut spend, record| {
            *spend.entry(record.unit.model.clone()).or_default() += cost(record);
            spend
        }),
        paused_until: None,
        stopped: None,
        failure: None,
    });
    let changed = Notify::new();
    log::info!("experimento: rodada {run_id} com {} teste(s) a rodar de {}", pending.len(), units.len());

    let snapshot = |status: RoundStatus| {
        let mut state = state.lock();
        let pause_left = state.paused_until.and_then(|until| until.checked_duration_since(Instant::now()));
        state.live.status = match (status, pause_left) {
            (RoundStatus::Running, Some(_)) => RoundStatus::Paused,
            _ => status,
        };
        state.live.paused_until = pause_left
            .map(|left| (chrono::Utc::now() + left).to_rfc3339_opts(chrono::SecondsFormat::Secs, true));
        state.live.updated_at = now();
        state.live.active = state.active.values().cloned().collect();
        let left = state.live.total - state.live.done;
        state.live.eta_seconds = (state.ran > 0 && left > 0)
            .then(|| (started.elapsed().as_secs_f64() / state.ran as f64 * left as f64).round() as u64);
        state.live.clone()
    };

    let work = futures_util::stream::iter(pending).for_each_concurrent(options.concurrency.max(1), |unit| {
        let (state, changed) = (&state, &changed);
        async move {
            // Uma pausa em curso segura o começo de qualquer teste
            loop {
                if stop.is_cancelled() {
                    return;
                }
                let left = state.lock().paused_until.and_then(|until| until.checked_duration_since(Instant::now()));
                match left {
                    Some(left) => tokio::time::sleep(left.min(Duration::from_secs(1))).await,
                    None => break,
                }
            }
            let key = unit.key();
            let observe = |live: &LiveUnit| {
                state.lock().active.insert(live.key.clone(), live.clone());
                changed.notify_one();
            };
            let record = runner.run(unit, stop, &observe).await;

            let (done, total) = {
                let mut state = state.lock();
                state.active.remove(&key);
                let written = match record.is_result() {
                    true => dir.append_unit(&record),
                    false => dir.append_discarded(&record),
                };
                if let Err(error) = written {
                    // Sem conseguir gravar não há por que seguir
                    state.failure = Some(error);
                    stop.cancel();
                } else {
                    if record.is_result() {
                        state.ran += 1;
                        state.live.done += 1;
                        match record.correct_final {
                            true => state.live.correct += 1,
                            false => state.live.incorrect += 1,
                        }
                    } else if record.status == NOT_OPENED {
                        state.live.not_opened += 1;
                    }
                    state.check(&record, options, stop);
                }
                (state.live.done, state.live.total)
            };
            changed.notify_one();
            report(&record, done, total);
        }
    });

    // O trabalho e a gravação do andamento, na mesma tarefa
    tokio::pin!(work);
    dir.write_live(&snapshot(RoundStatus::Running))?;
    loop {
        tokio::select! {
            _ = &mut work => break,
            _ = changed.notified() => {}
            _ = tokio::time::sleep(HEARTBEAT) => {}
        }
        dir.write_live(&snapshot(RoundStatus::Running))?;
        tokio::select! {
            _ = &mut work => break,
            _ = tokio::time::sleep(COALESCE) => {}
        }
    }

    let status = {
        let mut state = state.lock();
        state.paused_until = None;
        match (&state.stopped, stop.is_cancelled()) {
            (Some((status, reason)), _) => {
                let (status, reason) = (*status, reason.clone());
                state.live.stop_reason = Some(reason);
                status
            }
            (None, true) => RoundStatus::Interrupted,
            (None, false) => RoundStatus::Finished,
        }
    };
    let live = snapshot(status);
    dir.write_live(&live)?;
    let mut state = state.lock();
    if let Some(error) = state.failure.take() {
        return Err(error);
    }
    log::info!("experimento: rodada {run_id} parou ({status:?}), {} de {} feitos", live.done, live.total);
    Ok(Outcome {
        status,
        total: live.total,
        done: live.done,
        correct: live.correct,
        ran: state.ran,
        last_error: live.last_error,
        stop_reason: live.stop_reason,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::assistant::language::{Detected, Language};
    use crate::experiment::questions::Lang;
    use crate::experiment::record::CANCELLED;
    use crate::experiment::unit::{blank, Base};

    fn plan(count: usize) -> Vec<Unit> {
        (0..count)
            .map(|index| Unit {
                question_id: format!("q{index}"),
                paradigm: "postgres".into(),
                language: Lang::Pt,
                model: "modelo".into(),
                repetition: 1,
            })
            .collect()
    }

    /// Devolve o estado roteirizado de cada teste e anota quem rodou.
    #[derive(Default)]
    struct Stub {
        ran: Mutex<Vec<String>>,
        calls: AtomicUsize,
        /// Interrompe a rodada quando chega a este número de testes
        stop_at: Option<usize>,
        /// O estado de cada chamada, na ordem (o que faltar é `answered`);
        /// `failed:CODIGO` leva o código do erro
        statuses: Mutex<Vec<&'static str>>,
        /// O que cada teste custa, em dólares
        cost: f64,
    }

    #[async_trait]
    impl Runner for Stub {
        async fn run(&self, unit: &Unit, cancel: &CancelFlag, observe: Observer<'_>) -> UnitRecord {
            let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            self.ran.lock().push(unit.key());
            let mut record = blank(Base {
                unit,
                difficulty: "easy",
                dataset: None,
                unanswerable: false,
                question: "?",
                detected: Detected {
                    language: Language::Pt,
                    confidence: 1.0,
                },
            });
            observe(&LiveUnit {
                key: record.key.clone(),
                unit: unit.clone(),
                question: "?".into(),
                difficulty: "easy".into(),
                stage: crate::experiment::record::Stage::Generating,
                started_at: record.started_at.clone(),
                attempts: Vec::new(),
            });
            tokio::task::yield_now().await;
            let mut statuses = self.statuses.lock();
            let scripted = if statuses.is_empty() { "answered" } else { statuses.remove(0) };
            let (status, code) = scripted.split_once(':').unwrap_or((scripted, ""));
            record.status = status.to_string();
            record.error_code = (!code.is_empty()).then(|| code.to_string());
            record.correct_final = record.status == "answered";
            if self.cost > 0.0 {
                record.attempts.push(crate::experiment::record::AttemptRecord {
                    attempt_id: "a".into(),
                    index: 0,
                    kind: None,
                    query: None,
                    output: None,
                    raw: None,
                    generation_error: None,
                    counted: true,
                    failed_stage: None,
                    failure_class: None,
                    error: None,
                    outcome: None,
                    row_count: None,
                    correct: None,
                    exact_match: None,
                    usage: Some(crate::models::AssistantUsage {
                        input_tokens: 0,
                        output_tokens: 0,
                        cache_read_tokens: 0,
                        cache_write_tokens: 0,
                        cost_usd: self.cost,
                    }),
                    generation_ms: 0,
                });
            }
            if self.stop_at == Some(call) {
                cancel.cancel();
            }
            record
        }
    }

    fn dir(name: &str) -> (RunDir, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!("octapus-experiment-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let dir = RunDir::create(&path).unwrap();
        // Só para a pasta contar como rodada ao ser reaberta
        std::fs::write(path.join("manifest.json"), "{}").unwrap();
        (dir, path)
    }

    /// Sem pausa: três erros seguidos param direto
    fn options() -> Options {
        Options {
            pause: Duration::ZERO,
            ..Options::default()
        }
    }

    async fn round(stub: &Stub, units: &[Unit], dir: &RunDir, options: &Options) -> Outcome {
        run(stub, units, dir, "teste", options, &CancelFlag::default(), &|_, _, _| {}).await.unwrap()
    }

    #[tokio::test]
    async fn resumes_where_it_stopped_without_repeating() {
        let (dir, path) = dir("resume");
        let units = plan(10);

        let first = Stub {
            stop_at: Some(3),
            ..Default::default()
        };
        let outcome = round(&first, &units, &dir, &options()).await;
        assert_eq!((outcome.status, outcome.done, outcome.ran), (RoundStatus::Interrupted, 3, 3));
        // Sequencial e na ordem do plano
        assert_eq!(*first.ran.lock(), units[..3].iter().map(Unit::key).collect::<Vec<_>>());
        assert_eq!(dir.units().unwrap().len(), 3);

        // Outro processo retoma: abre a pasta de novo
        let dir = RunDir::open(&path).unwrap();
        let second = Stub::default();
        let outcome = round(&second, &units, &dir, &options()).await;
        assert_eq!((outcome.status, outcome.done, outcome.ran), (RoundStatus::Finished, 10, 7));
        assert_eq!(*second.ran.lock(), units[3..].iter().map(Unit::key).collect::<Vec<_>>());
        assert_eq!(dir.units().unwrap().len(), 10);

        // Nada a fazer numa rodada completa
        let third = Stub::default();
        let outcome = round(&third, &units, &dir, &options()).await;
        assert_eq!((outcome.status, outcome.ran, third.calls.load(Ordering::SeqCst)), (RoundStatus::Finished, 0, 0));
        let _ = std::fs::remove_dir_all(path);
    }

    #[tokio::test]
    async fn interrupted_and_unopened_tests_run_again() {
        let (dir, path) = dir("discarded");
        let units = plan(4);

        let first = Stub {
            statuses: Mutex::new(vec!["answered", CANCELLED, NOT_OPENED]),
            ..Default::default()
        };
        let outcome = round(&first, &units, &dir, &options()).await;
        // Só os dois que terminaram valem
        assert_eq!((outcome.status, outcome.done, outcome.ran), (RoundStatus::Finished, 2, 2));
        let live: Live = serde_json::from_str(&std::fs::read_to_string(path.join("live.json")).unwrap()).unwrap();
        assert_eq!((live.done, live.total, live.not_opened, live.active.len()), (2, 4, 1, 0));
        assert_eq!(std::fs::read_to_string(path.join("discarded.jsonl")).unwrap().lines().count(), 2);

        let second = Stub::default();
        let outcome = round(&second, &units, &dir, &options()).await;
        assert_eq!((outcome.done, outcome.ran), (4, 2));
        assert_eq!(*second.ran.lock(), [units[1].key(), units[2].key()]);
        let _ = std::fs::remove_dir_all(path);
    }

    #[tokio::test]
    async fn stops_after_consecutive_unopened_tests() {
        let (dir, path) = dir("errors");
        let units = plan(8);
        let stub = Stub {
            statuses: Mutex::new(vec!["answered", NOT_OPENED, "answered", NOT_OPENED, NOT_OPENED, NOT_OPENED]),
            ..Default::default()
        };
        let outcome = round(&stub, &units, &dir, &options()).await;
        // Um erro isolado não para; três seguidos param, e o resto não roda
        assert_eq!((outcome.status, outcome.done), (RoundStatus::StoppedOnErrors, 2));
        assert_eq!(stub.calls.load(Ordering::SeqCst), 6);
        let _ = std::fs::remove_dir_all(path);
    }

    /// O turno que falha no meio conta como erro seguido, e a rodada pausa e
    /// tenta de novo antes de desistir.
    #[tokio::test]
    async fn pauses_after_consecutive_failures_and_goes_on() {
        let (dir, path) = dir("pause");
        let units = plan(8);
        let stub = Stub {
            statuses: Mutex::new(vec!["answered", FAILED, NOT_OPENED, FAILED]),
            ..Default::default()
        };
        let pausing = Options {
            pause: Duration::from_millis(30),
            max_pauses: 1,
            failure_rate_after: 100,
            ..Options::default()
        };
        let started = Instant::now();
        let outcome = round(&stub, &units, &dir, &pausing).await;
        assert!(started.elapsed() >= Duration::from_millis(30), "esperou a pausa");
        // Seguiu depois da pausa e fez tudo; o que não abriu volta na retomada
        assert_eq!((outcome.status, outcome.done, stub.calls.load(Ordering::SeqCst)), (RoundStatus::Finished, 7, 8));
        let live: Live = serde_json::from_str(&std::fs::read_to_string(path.join("live.json")).unwrap()).unwrap();
        assert_eq!((live.pauses, live.paused_until, live.stop_reason), (1, None, None));
        let _ = std::fs::remove_dir_all(path);
    }

    #[tokio::test]
    async fn stops_when_errors_go_on_after_every_pause() {
        let (dir, path) = dir("pauses");
        let stub = Stub {
            statuses: Mutex::new(vec![FAILED; 20]),
            ..Default::default()
        };
        let pausing = Options {
            pause: Duration::from_millis(5),
            max_pauses: 2,
            failure_rate_after: 100,
            ..Options::default()
        };
        let outcome = round(&stub, &plan(20), &dir, &pausing).await;
        // 3 erros e pausa, 3 e pausa, 3 e para
        assert_eq!((outcome.status, stub.calls.load(Ordering::SeqCst)), (RoundStatus::StoppedOnErrors, 9));
        assert!(outcome.stop_reason.unwrap().starts_with("3 erros seguidos depois de 2 pausa(s)"));
        let _ = std::fs::remove_dir_all(path);
    }

    #[tokio::test]
    async fn a_fatal_error_stops_at_once() {
        let (dir, path) = dir("fatal");
        let stub = Stub {
            statuses: Mutex::new(vec!["answered", "failed:ASSISTANT_QUOTA_EXCEEDED"]),
            ..Default::default()
        };
        let outcome = round(&stub, &plan(6), &dir, &Options::default()).await;
        assert_eq!((outcome.status, stub.calls.load(Ordering::SeqCst)), (RoundStatus::StoppedOnErrors, 2));
        assert!(outcome.stop_reason.unwrap().contains("ASSISTANT_QUOTA_EXCEEDED"));
        let live: Live = serde_json::from_str(&std::fs::read_to_string(path.join("live.json")).unwrap()).unwrap();
        assert_eq!(live.status, RoundStatus::StoppedOnErrors);
        assert!(live.stop_reason.unwrap().contains("ASSISTANT_QUOTA_EXCEEDED"));
        let _ = std::fs::remove_dir_all(path);
    }

    /// Falhas espalhadas, que nunca chegam a três seguidas, também param.
    #[tokio::test]
    async fn stops_when_too_many_tests_fail() {
        let (dir, path) = dir("rate");
        let stub = Stub {
            statuses: Mutex::new(vec!["answered", FAILED, "answered", FAILED, "answered", FAILED, "answered", FAILED]),
            ..Default::default()
        };
        let strict = Options {
            max_failure_rate: 0.4,
            failure_rate_after: 4,
            ..options()
        };
        let outcome = round(&stub, &plan(10), &dir, &strict).await;
        // No 4º teste: 2 de 4 (50%) > 40%
        assert_eq!((outcome.status, stub.calls.load(Ordering::SeqCst)), (RoundStatus::StoppedOnErrors, 4));
        assert!(outcome.stop_reason.unwrap().starts_with("2 de 4 testes desta execução falharam (50%"));
        let _ = std::fs::remove_dir_all(path);
    }

    /// O teto de custo conta o que já foi gasto em execuções anteriores.
    #[tokio::test]
    async fn stops_at_the_cost_ceiling_of_a_model() {
        let (dir, path) = dir("budget");
        let units = plan(10);
        let capped = Options {
            max_cost_usd: BTreeMap::from([("modelo".to_string(), 1.0)]),
            ..options()
        };
        let first = Stub {
            cost: 0.3,
            ..Default::default()
        };
        let outcome = round(&first, &units, &dir, &capped).await;
        // 0,3 + 0,3 + 0,3 + 0,3 passa de 1,0 no 4º
        assert_eq!((outcome.status, outcome.done), (RoundStatus::StoppedOnBudget, 4));
        assert!(outcome.stop_reason.unwrap().contains("modelo chegou ao teto de custo (US$ 1.20 de US$ 1.00)"));

        // A retomada já começa no teto
        let dir = RunDir::open(&path).unwrap();
        let second = Stub {
            cost: 0.3,
            ..Default::default()
        };
        let outcome = round(&second, &units, &dir, &capped).await;
        assert_eq!((outcome.status, second.calls.load(Ordering::SeqCst)), (RoundStatus::StoppedOnBudget, 1));
        let _ = std::fs::remove_dir_all(path);
    }

    #[tokio::test]
    async fn failed_turns_only_run_again_when_asked() {
        let (dir, path) = dir("redo");
        let units = plan(3);
        let first = Stub {
            statuses: Mutex::new(vec!["answered", FAILED]),
            ..Default::default()
        };
        assert_eq!(round(&first, &units, &dir, &options()).await.done, 3);

        let again = Stub::default();
        assert_eq!(round(&again, &units, &dir, &options()).await.ran, 0);

        let redo = Stub::default();
        let outcome = round(
            &redo,
            &units,
            &dir,
            &Options {
                redo_failed: true,
                ..options()
            },
        )
        .await;
        assert_eq!((outcome.ran, outcome.done, outcome.correct), (1, 3, 3));
        assert_eq!(*redo.ran.lock(), [units[1].key()]);
        // O registro novo vale; o antigo continua no arquivo
        let records = dir.units().unwrap();
        assert_eq!(records.len(), 3);
        assert_eq!(records[1].status, "answered");
        assert_eq!(std::fs::read_to_string(path.join("units.jsonl")).unwrap().lines().count(), 4);
        let _ = std::fs::remove_dir_all(path);
    }

    #[tokio::test]
    async fn runs_tests_concurrently_when_asked() {
        let (dir, path) = dir("concurrency");
        let units = plan(9);
        let stub = Stub::default();
        let outcome = round(
            &stub,
            &units,
            &dir,
            &Options {
                concurrency: 4,
                ..options()
            },
        )
        .await;
        assert_eq!((outcome.status, outcome.done), (RoundStatus::Finished, 9));
        let keys: HashSet<String> = dir.units().unwrap().into_iter().map(|record| record.key).collect();
        assert_eq!(keys.len(), 9);
        let _ = std::fs::remove_dir_all(path);
    }

    /// Um processo morto no meio da escrita deixa uma linha cortada: ela é
    /// ignorada e não estraga a próxima.
    #[tokio::test]
    async fn a_truncated_line_is_skipped_and_does_not_break_the_next() {
        let (dir, path) = dir("truncated");
        let units = plan(3);
        let first = Stub {
            stop_at: Some(1),
            ..Default::default()
        };
        round(&first, &units, &dir, &options()).await;
        let file = path.join("units.jsonl");
        let mut text = std::fs::read_to_string(&file).unwrap();
        text.push_str("{\"key\":\"q1|postgres|pt|modelo|r1\",\"questi");
        std::fs::write(&file, text).unwrap();

        let dir = RunDir::open(&path).unwrap();
        assert_eq!(dir.units().unwrap().len(), 1);
        let second = Stub::default();
        let outcome = round(&second, &units, &dir, &options()).await;
        assert_eq!((outcome.done, outcome.ran), (3, 2));
        assert_eq!(dir.units().unwrap().len(), 3);
        let _ = std::fs::remove_dir_all(path);
    }
}
