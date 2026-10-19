//! Jobs de backup/restauração: um processo das ferramentas do Postgres por
//! job, com a saída virando log (eventos em lote), cancelamento e o arquivo
//! gravado em `.partial` até dar certo.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::ChildStdin;
use tokio::sync::{mpsc, oneshot};

use crate::models::{BackupEvent, BackupEventKind};

use super::args::{error_message, is_error_line, is_ignored_errors_line, keep_input_line};
use super::tool_command;

/// Para onde vão os eventos (na app, o `emit` do Tauri).
pub type BackupSink = Arc<dyn Fn(BackupEvent) + Send + Sync>;

/// O log sai em lotes: no `--verbose` de um database grande são milhares de
/// linhas, uma por objeto — um evento por linha afogaria o front.
const LOG_FLUSH_EVERY: Duration = Duration::from_millis(200);
const LOG_FLUSH_LINES: usize = 200;
/// Linhas guardadas para montar a mensagem de erro.
const ERROR_TAIL: usize = 30;
/// Linhas de erro guardadas (as mensagens finais e os avisos da restauração).
const MAX_ERRORS: usize = 100;

/// Uma execução a fazer.
pub struct JobSpec {
    /// `pg_dump`, `pg_restore` ou `psql` — nas mensagens
    pub tool: &'static str,
    pub program: PathBuf,
    pub args: Vec<String>,
    /// Variáveis do ambiente do filho (senha, `PGSSLMODE`, …). As `PG*` do
    /// processo do app são limpas antes, para não vazarem na conexão.
    pub env: Vec<(String, String)>,
    /// `(parcial, final)`: o processo grava no parcial, que vira o final só no
    /// sucesso (e some no erro/cancelamento). `None` quando não gera arquivo.
    pub output: Option<(PathBuf, PathBuf)>,
    /// Saída com erro conta como sucesso com avisos quando a ferramenta diz
    /// que ignorou erros e seguiu (`pg_restore` sem `--exit-on-error`)
    pub tolerate_ignored_errors: bool,
    /// Roda depois de um `Done` (na restauração, revalidar o catálogo)
    pub on_success: Option<Box<dyn FnOnce() + Send>>,
    /// O que vai no stdin do processo, filtrado (ver [`keep_input_line`])
    pub input: Option<JobInput>,
}

/// Entrada do processo principal (o `psql` lendo de `--file=-`).
pub enum JobInput {
    /// Um script SQL do disco
    File(PathBuf),
    /// A saída de outro processo (`pg_restore --file=-` gerando o SQL)
    Process {
        tool: &'static str,
        program: PathBuf,
        args: Vec<String>,
    },
}

#[derive(Default)]
pub struct BackupService {
    sink: OnceLock<BackupSink>,
    /// Jobs em andamento → gatilho de cancelamento
    jobs: Arc<Mutex<HashMap<String, oneshot::Sender<()>>>>,
    next_id: AtomicU64,
}

impl BackupService {
    /// Liga os eventos ao destino (uma vez, na partida do app).
    pub fn init(&self, sink: BackupSink) {
        let _ = self.sink.set(sink);
    }

    /// Começa um job em segundo plano e devolve o id dele; o andamento chega
    /// pelos eventos.
    pub fn start(&self, spec: JobSpec) -> String {
        let job_id = format!("job-{}", self.next_id.fetch_add(1, Ordering::Relaxed) + 1);
        let (cancel_tx, cancel_rx) = oneshot::channel();
        self.jobs.lock().insert(job_id.clone(), cancel_tx);

        let sink = self.sink.get().cloned();
        let jobs = Arc::clone(&self.jobs);
        let id = job_id.clone();
        let tool = spec.tool;
        log::info!("{job_id}: {tool} iniciado");
        tokio::spawn(async move {
            let emit = |kind| {
                if let Some(sink) = &sink {
                    sink(BackupEvent {
                        job_id: id.clone(),
                        kind,
                    });
                }
            };
            let outcome = run(spec, cancel_rx, &emit).await;
            jobs.lock().remove(&id);
            match &outcome {
                BackupEventKind::Done {
                    size_bytes,
                    elapsed_ms,
                    warnings,
                    ..
                } => log::info!(
                    "{id}: {tool} concluído em {elapsed_ms} ms ({size_bytes} bytes, {} aviso(s))",
                    warnings.len()
                ),
                BackupEventKind::Error { message } => log::warn!("{id}: {tool} falhou: {message}"),
                BackupEventKind::Cancelled => log::info!("{id}: {tool} cancelado"),
                BackupEventKind::Log { .. } => {}
            }
            emit(outcome);
        });
        job_id
    }

    /// Pede o cancelamento; `false` se o job já terminou (ou nunca existiu).
    pub fn cancel(&self, job_id: &str) -> bool {
        match self.jobs.lock().remove(job_id) {
            Some(cancel) => cancel.send(()).is_ok(),
            None => false,
        }
    }
}

/// Uma linha de saída: do stdout do processo principal (só log) ou de um
/// stderr (log, e é ali que os erros aparecem).
enum Line {
    Out(String),
    Err(String),
}

/// Limpa as `PG*` herdadas e põe as do job.
fn prepare(cmd: &mut tokio::process::Command, env: &[(String, String)]) {
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("PG") {
            cmd.env_remove(key);
        }
    }
    cmd.envs(env.iter().map(|(k, v)| (k, v)));
}

/// Manda cada linha de `reader` para o canal (bytes inválidos em UTF-8 não
/// derrubam a leitura).
fn forward<R>(reader: R, tx: mpsc::UnboundedSender<Line>, wrap: fn(String) -> Line)
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut reader = BufReader::new(reader);
        let mut buf = Vec::new();
        loop {
            buf.clear();
            match reader.read_until(b'\n', &mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    let line = String::from_utf8_lossy(&buf);
                    let line = line.trim_end_matches(['\n', '\r']).to_string();
                    if tx.send(wrap(line)).is_err() {
                        break;
                    }
                }
            }
        }
    });
}

/// Copia `source` para `sink` linha a linha, sem as linhas que o filtro
/// descarta. `false` se o destino fechou antes (o processo saiu).
async fn copy_filtered<R, W>(source: R, sink: &mut W) -> std::io::Result<bool>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    let mut reader = BufReader::new(source);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        if reader.read_until(b'\n', &mut buf).await? == 0 {
            break;
        }
        if !keep_input_line(&buf) {
            continue;
        }
        if sink.write_all(&buf).await.is_err() {
            return Ok(false);
        }
    }
    Ok(sink.flush().await.is_ok())
}

/// Alimenta o stdin do processo principal. Se a fonte falhar, avisa por
/// `abort` e segura o stdin aberto: o processo é morto antes de ver o fim da
/// entrada (no "tudo ou nada", um EOF faria o `psql` confirmar o que chegou).
async fn feed(
    input: JobInput,
    mut stdin: ChildStdin,
    env: Vec<(String, String)>,
    lines: mpsc::UnboundedSender<Line>,
    abort: oneshot::Sender<String>,
) {
    let failure = match input {
        JobInput::File(path) => match tokio::fs::File::open(&path).await {
            Ok(file) => match copy_filtered(file, &mut stdin).await {
                Ok(_) => None,
                Err(e) => Some(format!("Não foi possível ler {}: {e}", path.display())),
            },
            Err(e) => Some(format!("Não foi possível abrir {}: {e}", path.display())),
        },
        JobInput::Process {
            tool,
            program,
            args,
        } => {
            let mut cmd = tool_command(&program);
            prepare(&mut cmd, &env);
            cmd.args(&args)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped());
            match cmd.spawn() {
                Err(e) => Some(format!("Não foi possível executar o {tool}: {e}")),
                Ok(mut producer) => {
                    forward(
                        producer.stderr.take().expect("stderr piped"),
                        lines,
                        Line::Err,
                    );
                    let stdout = producer.stdout.take().expect("stdout piped");
                    match copy_filtered(stdout, &mut stdin).await {
                        // O destino saiu antes (erro dele): o produtor morre no drop
                        Ok(false) => None,
                        Ok(true) => match producer.wait().await {
                            Ok(status) if status.success() => None,
                            Ok(status) => Some(match status.code() {
                                Some(code) => format!("{tool} terminou com o código {code}."),
                                None => format!("{tool} foi interrompido."),
                            }),
                            Err(e) => Some(format!("{tool} falhou: {e}")),
                        },
                        Err(e) => Some(format!("Não foi possível ler a saída do {tool}: {e}")),
                    }
                }
            }
        }
    };
    match failure {
        None => drop(stdin),
        Some(message) => {
            let _ = abort.send(message);
            // Quem roda o job mata o processo e então cancela esta task
            std::future::pending::<()>().await;
        }
    }
}

/// Roda o processo até o fim e devolve o evento final (`Done`, `Error` ou
/// `Cancelled`); os `Log` saem pelo caminho.
async fn run(
    spec: JobSpec,
    mut cancel: oneshot::Receiver<()>,
    emit: &impl Fn(BackupEventKind),
) -> BackupEventKind {
    let started = Instant::now();
    let tool = spec.tool;

    let mut cmd = tool_command(&spec.program);
    prepare(&mut cmd, &spec.env);
    cmd.args(&spec.args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if spec.input.is_some() {
        cmd.stdin(Stdio::piped());
    }

    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => {
            return BackupEventKind::Error {
                message: format!("Não foi possível executar o {tool}: {e}"),
            }
        }
    };

    // O log é o stderr (o `--verbose` e os erros) e o stdout (o `psql` escreve
    // `CREATE TABLE`, `COPY 120`…; o `pg_dump` com `--file` não escreve nada),
    // mais o stderr de quem alimenta a entrada
    let (lines_tx, mut lines) = mpsc::unbounded_channel();
    forward(
        child.stderr.take().expect("stderr piped"),
        lines_tx.clone(),
        Line::Err,
    );
    forward(
        child.stdout.take().expect("stdout piped"),
        lines_tx.clone(),
        Line::Out,
    );
    let (abort_tx, mut abort) = oneshot::channel::<String>();
    let feeder = spec.input.map(|input| {
        let stdin = child.stdin.take().expect("stdin piped");
        tokio::spawn(feed(
            input,
            stdin,
            spec.env.clone(),
            lines_tx.clone(),
            abort_tx,
        ))
    });
    drop(lines_tx);
    // O oneshot não pode ser lido de novo depois de resolvido
    let mut abort_open = feeder.is_some();

    let mut batch = Vec::new();
    let mut tail = VecDeque::with_capacity(ERROR_TAIL);
    let mut errors = Vec::new();
    let mut ignored_errors = false;
    let mut tick = tokio::time::interval(LOG_FLUSH_EVERY);
    let flush = |batch: &mut Vec<String>| {
        if !batch.is_empty() {
            emit(BackupEventKind::Log {
                lines: std::mem::take(batch),
            });
        }
    };

    enum Stop {
        Eof,
        Cancelled,
        Aborted(String),
    }
    let stop = loop {
        tokio::select! {
            _ = &mut cancel => break Stop::Cancelled,
            message = &mut abort, if abort_open => match message {
                Ok(message) => break Stop::Aborted(message),
                // O feeder terminou bem (sem abortar): só não há mais o que ouvir aqui
                Err(_) => abort_open = false,
            },
            line = lines.recv() => match line {
                Some(Line::Err(line)) => {
                    if is_ignored_errors_line(&line) {
                        ignored_errors = true;
                    } else if is_error_line(&line) && errors.len() < MAX_ERRORS {
                        errors.push(line.clone());
                    }
                    if tail.len() == ERROR_TAIL {
                        tail.pop_front();
                    }
                    tail.push_back(line.clone());
                    batch.push(line);
                }
                Some(Line::Out(line)) => batch.push(line),
                // Todas as saídas fecharam: só falta esperar o fim do processo
                None => break Stop::Eof,
            },
            _ = tick.tick() => flush(&mut batch),
        }
        if batch.len() >= LOG_FLUSH_LINES {
            flush(&mut batch);
        }
    };
    flush(&mut batch);

    let status: Result<ExitStatus, Stop> = match stop {
        Stop::Eof => tokio::select! {
            _ = &mut cancel => Err(Stop::Cancelled),
            message = &mut abort, if abort_open => match message {
                Ok(message) => Err(Stop::Aborted(message)),
                Err(_) => child.wait().await.map_err(|e| Stop::Aborted(e.to_string())),
            },
            status = child.wait() => status.map_err(|e| Stop::Aborted(e.to_string())),
        },
        other => Err(other),
    };
    let status = match status {
        Ok(status) => status,
        Err(stop) => {
            // O processo morre antes de o stdin fechar (a task do feeder é
            // cancelada depois): nada da entrada parcial é confirmado
            let _ = child.kill().await;
            if let Some(feeder) = &feeder {
                feeder.abort();
            }
            discard_partial(&spec.output).await;
            return match stop {
                Stop::Aborted(message) => {
                    let mut lines = errors;
                    lines.push(message);
                    BackupEventKind::Error {
                        message: error_message(tool, &lines, None),
                    }
                }
                _ => BackupEventKind::Cancelled,
            };
        }
    };

    // O `pg_restore` sem "tudo ou nada" segue depois de um erro e sai com 1
    // avisando quantos ignorou: a restauração aconteceu, com avisos
    let tolerated = spec.tolerate_ignored_errors && ignored_errors;
    if !status.success() && !tolerated {
        discard_partial(&spec.output).await;
        let lines: Vec<String> = if errors.is_empty() {
            tail.into()
        } else {
            errors
        };
        return BackupEventKind::Error {
            message: error_message(tool, &lines, status.code()),
        };
    }

    let elapsed_ms = started.elapsed().as_millis() as u64;
    let done = match spec.output {
        None => BackupEventKind::Done {
            path: String::new(),
            size_bytes: 0,
            elapsed_ms,
            warnings: errors,
        },
        Some((partial, path)) => match finish_file(&partial, &path).await {
            Ok(size_bytes) => BackupEventKind::Done {
                path: path.display().to_string(),
                size_bytes,
                elapsed_ms,
                warnings: errors,
            },
            Err(message) => {
                let _ = tokio::fs::remove_file(&partial).await;
                return BackupEventKind::Error { message };
            }
        },
    };
    if let Some(on_success) = spec.on_success {
        on_success();
    }
    done
}

/// Põe o parcial no lugar do final (substituindo, que o diálogo de salvar já
/// confirmou) e devolve o tamanho.
async fn finish_file(partial: &Path, path: &Path) -> Result<u64, String> {
    let size = tokio::fs::metadata(partial)
        .await
        .map_err(|e| format!("O arquivo gerado sumiu ({}): {e}", partial.display()))?
        .len();
    // No Windows o rename não substitui um arquivo que já existe
    #[cfg(windows)]
    let _ = tokio::fs::remove_file(path).await;
    tokio::fs::rename(partial, path)
        .await
        .map_err(|e| format!("Não foi possível gravar {}: {e}", path.display()))?;
    Ok(size)
}

async fn discard_partial(output: &Option<(PathBuf, PathBuf)>) {
    if let Some((partial, _)) = output {
        let _ = tokio::fs::remove_file(partial).await;
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    /// Pasta temporária própria do teste, apagada no fim.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("octapus-backup-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Um "pg_dump" em shell: grava `$1` com o conteúdo e loga no stderr.
    fn script(dir: &TempDir, body: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.0.join("fake_tool");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn spec(dir: &TempDir, program: PathBuf) -> (JobSpec, PathBuf, PathBuf) {
        let path = dir.0.join("saida.dump");
        let partial = dir.0.join("saida.dump.partial");
        let spec = JobSpec {
            tool: "pg_dump",
            program,
            args: vec![partial.display().to_string()],
            env: vec![("PGPASSWORD".into(), "s3gredo".into())],
            output: Some((partial.clone(), path.clone())),
            tolerate_ignored_errors: false,
            on_success: None,
            input: None,
        };
        (spec, partial, path)
    }

    /// Roda o job pelo serviço e junta os eventos até o final.
    async fn run_job(spec: JobSpec, cancel_after: Option<Duration>) -> Vec<BackupEventKind> {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let service = BackupService::default();
        service.init(Arc::new(move |event: BackupEvent| {
            let _ = tx.send(event);
        }));
        let job_id = service.start(spec);
        if let Some(delay) = cancel_after {
            tokio::time::sleep(delay).await;
            assert!(service.cancel(&job_id));
        }
        let mut kinds = Vec::new();
        while let Some(event) = rx.recv().await {
            assert_eq!(event.job_id, job_id);
            let last = !matches!(event.kind, BackupEventKind::Log { .. });
            kinds.push(event.kind);
            if last {
                break;
            }
        }
        // Terminado, o job sai da lista
        assert!(!service.cancel(&job_id));
        kinds
    }

    fn log_lines(kinds: &[BackupEventKind]) -> Vec<String> {
        kinds
            .iter()
            .filter_map(|kind| match kind {
                BackupEventKind::Log { lines } => Some(lines.clone()),
                _ => None,
            })
            .flatten()
            .collect()
    }

    #[tokio::test]
    async fn success_moves_the_partial_into_place() {
        let dir = TempDir::new("ok");
        let program = script(
            &dir,
            "echo 'pg_dump: dumping contents of table \"public.pedidos\"' >&2\n\
             printf \"senha=$PGPASSWORD\" > \"$1\"",
        );
        let (spec, partial, path) = spec(&dir, program);

        let kinds = run_job(spec, None).await;
        assert_eq!(
            log_lines(&kinds),
            vec!["pg_dump: dumping contents of table \"public.pedidos\""]
        );
        match kinds.last().unwrap() {
            BackupEventKind::Done {
                path: done,
                size_bytes,
                ..
            } => {
                assert_eq!(done, &path.display().to_string());
                assert_eq!(*size_bytes, "senha=s3gredo".len() as u64);
            }
            other => panic!("esperava Done, veio {other:?}"),
        }
        // A senha chegou pelo ambiente
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "senha=s3gredo");
        assert!(!partial.exists());
    }

    #[tokio::test]
    async fn failure_reports_the_error_and_leaves_no_file() {
        let dir = TempDir::new("erro");
        let program = script(
            &dir,
            "echo parcial > \"$1\"\n\
             echo 'pg_dump: error: connection to server failed: FATAL:  password authentication failed' >&2\n\
             exit 1",
        );
        let (spec, partial, path) = spec(&dir, program);

        let kinds = run_job(spec, None).await;
        assert_eq!(
            kinds.last().unwrap(),
            &BackupEventKind::Error {
                message: "pg_dump: error: connection to server failed: FATAL:  password authentication failed".into()
            }
        );
        assert!(!partial.exists());
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn cancel_kills_the_process_and_leaves_no_file() {
        let dir = TempDir::new("cancel");
        let program = script(&dir, "echo parcial > \"$1\"\nexec sleep 30");
        let (spec, partial, path) = spec(&dir, program);

        let started = Instant::now();
        let kinds = run_job(spec, Some(Duration::from_millis(300))).await;
        assert_eq!(kinds.last().unwrap(), &BackupEventKind::Cancelled);
        assert!(started.elapsed() < Duration::from_secs(10));
        assert!(!partial.exists());
        assert!(!path.exists());
    }

    #[tokio::test]
    async fn inherited_pg_variables_do_not_leak() {
        let dir = TempDir::new("env");
        // PGHOST do ambiente do app não pode chegar ao filho; o do spec chega
        std::env::set_var("PGHOST_OCTAPUS_TEST", "vazou");
        let program = script(
            &dir,
            "printf \"[$PGHOST_OCTAPUS_TEST][$PGPASSWORD]\" > \"$1\"",
        );
        let (spec, _, path) = spec(&dir, program);

        run_job(spec, None).await;
        std::env::remove_var("PGHOST_OCTAPUS_TEST");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "[][s3gredo]");
    }

    /// Um "pg_restore" sem arquivo de saída, como a restauração usa.
    fn restore_spec(program: PathBuf, tolerate: bool) -> (JobSpec, Arc<AtomicU64>) {
        let calls = Arc::new(AtomicU64::new(0));
        let counter = Arc::clone(&calls);
        let spec = JobSpec {
            tool: "pg_restore",
            program,
            args: vec![],
            env: vec![],
            output: None,
            tolerate_ignored_errors: tolerate,
            on_success: Some(Box::new(move || {
                counter.fetch_add(1, Ordering::SeqCst);
            })),
            input: None,
        };
        (spec, calls)
    }

    #[tokio::test]
    async fn ignored_restore_errors_become_warnings() {
        let dir = TempDir::new("ignorados");
        let program = script(
            &dir,
            "echo 'pg_restore: creating TABLE \"public.pedidos\"' >&2\n\
             echo 'pg_restore: error: could not execute query: ERROR:  role \"app\" does not exist' >&2\n\
             echo 'pg_restore: warning: errors ignored on restore: 1' >&2\n\
             exit 1",
        );

        let (spec, calls) = restore_spec(program.clone(), true);
        match run_job(spec, None).await.last().unwrap() {
            BackupEventKind::Done { warnings, path, .. } => {
                assert_eq!(
                    warnings,
                    &vec!["pg_restore: error: could not execute query: ERROR:  role \"app\" does not exist".to_string()]
                );
                assert!(path.is_empty());
            }
            other => panic!("esperava Done, veio {other:?}"),
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        // Em "tudo ou nada" a mesma saída é erro, e o catálogo não é tocado
        let (spec, calls) = restore_spec(program, false);
        assert!(matches!(
            run_job(spec, None).await.last().unwrap(),
            BackupEventKind::Error { .. }
        ));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn stdout_lines_also_go_to_the_log() {
        let dir = TempDir::new("stdout");
        let program = script(&dir, "echo 'CREATE TABLE'\necho 'COPY 120'");
        let (spec, calls) = restore_spec(program, false);

        let kinds = run_job(spec, None).await;
        assert_eq!(log_lines(&kinds), vec!["CREATE TABLE", "COPY 120"]);
        assert!(
            matches!(kinds.last().unwrap(), BackupEventKind::Done { warnings, .. } if warnings.is_empty())
        );
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn missing_program_is_an_error() {
        let dir = TempDir::new("sem-programa");
        let (spec, _, _) = spec(&dir, dir.0.join("nao_existe"));
        match run_job(spec, None).await.last().unwrap() {
            BackupEventKind::Error { message } => {
                assert!(message.starts_with("Não foi possível executar o pg_dump"))
            }
            other => panic!("esperava Error, veio {other:?}"),
        }
    }
}
