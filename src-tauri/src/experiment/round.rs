//! O controle de uma rodada: roda os testes que faltam na ordem do plano, com
//! a concorrência pedida, grava cada um ao terminar e mantém o `live.json`.
//!
//! - **Retomar** é rodar de novo sobre a mesma pasta: o que já está no
//!   `units.jsonl` é pulado.
//! - **Interromper** (o [`CancelFlag`] da rodada) para os turnos em andamento
//!   pelo cancelamento do próprio assistente e não começa outros. Um teste
//!   interrompido não vale como resultado: volta na retomada.
//! - **Erros seguidos** de teste que nem abriu (API fora, cota esgotada) param
//!   a rodada sozinha, para não gravar uma fila de falhas que não são do
//!   modelo.

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

/// Quem roda um teste: o laboratório de verdade (`lab::Lab`) ou um simulado.
#[async_trait]
pub trait Runner: Send + Sync {
    async fn run(&self, unit: &Unit, cancel: &CancelFlag, observe: Observer<'_>) -> UnitRecord;
}

pub struct Options {
    pub concurrency: usize,
    pub max_consecutive_errors: u32,
    /// Roda de novo os testes cujo turno terminou em `failed`
    pub redo_failed: bool,
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
}

struct State {
    live: Live,
    active: BTreeMap<String, LiveUnit>,
    consecutive_errors: u32,
    ran: usize,
    stopped_on_errors: bool,
    failure: Option<io::Error>,
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
        },
        active: BTreeMap::new(),
        consecutive_errors: 0,
        ran: 0,
        stopped_on_errors: false,
        failure: None,
    });
    let changed = Notify::new();
    log::info!("experimento: rodada {run_id} com {} teste(s) a rodar de {}", pending.len(), units.len());

    let snapshot = |status: RoundStatus| {
        let mut state = state.lock();
        state.live.status = status;
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
            if stop.is_cancelled() {
                return;
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
                } else if record.is_result() {
                    state.ran += 1;
                    state.consecutive_errors = 0;
                    state.live.done += 1;
                    match record.correct_final {
                        true => state.live.correct += 1,
                        false => state.live.incorrect += 1,
                    }
                } else if record.status == NOT_OPENED {
                    state.live.not_opened += 1;
                    state.live.last_error = record.error_code.clone();
                    state.consecutive_errors += 1;
                    if state.consecutive_errors >= options.max_consecutive_errors {
                        state.stopped_on_errors = true;
                        stop.cancel();
                    }
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
        let state = state.lock();
        match (state.stopped_on_errors, stop.is_cancelled()) {
            (true, _) => RoundStatus::StoppedOnErrors,
            (false, true) => RoundStatus::Interrupted,
            (false, false) => RoundStatus::Finished,
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
        /// O estado de cada chamada, na ordem (o que faltar é `answered`)
        statuses: Mutex<Vec<&'static str>>,
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
            record.status = if statuses.is_empty() { "answered" } else { statuses.remove(0) }.to_string();
            record.correct_final = record.status == "answered";
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

    fn options() -> Options {
        Options {
            concurrency: 1,
            max_consecutive_errors: 3,
            redo_failed: false,
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
