//! O log do `octapus`: um arquivo na mesma pasta de logs do app
//! (`octapus-cli.log`), nunca o stdout nem o stderr — a saída do comando é
//! dado, e a TUI ocupa a tela.
//!
//! Os níveis são os do app (`octapus_core::logging`): o que é nosso em
//! `info`, as dependências só a partir de `warn`. A regra do conteúdo também:
//! nunca senha, URI, texto de query nem nomes de schema ou tabela.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use log::{LevelFilter, Log, Metadata, Record};
use octapus_core::logging::{app_level, CORE_TARGET, DEPENDENCIES_LEVEL};
use parking_lot::Mutex;

const FILE_NAME: &str = "octapus-cli.log";
/// Passou disto na partida, o arquivo vira `.1` (um só antigo é guardado).
const MAX_FILE_BYTES: u64 = 5 * 1024 * 1024;
/// O target dos logs deste binário.
const CLI_TARGET: &str = "octapus";

struct FileLogger {
    file: Mutex<File>,
}

fn level_for(target: &str) -> LevelFilter {
    if target.starts_with(CORE_TARGET) || target.starts_with(CLI_TARGET) {
        app_level()
    } else {
        DEPENDENCIES_LEVEL
    }
}

impl Log for FileLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= level_for(metadata.target())
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let seconds = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_secs());
        // Falha ao gravar o log não derruba o comando
        let _ = writeln!(
            self.file.lock(),
            "[{seconds}][{}][{}][{}] {}",
            std::process::id(),
            record.level(),
            record.target(),
            record.args()
        );
    }

    fn flush(&self) {
        let _ = self.file.lock().flush();
    }
}

/// Liga o log em `dir`. Sem conseguir abrir o arquivo, o comando segue sem
/// log (um `octapus --help` numa máquina só de leitura tem que funcionar).
pub fn init(dir: &Path) {
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let path = dir.join(FILE_NAME);
    if std::fs::metadata(&path).is_ok_and(|meta| meta.len() > MAX_FILE_BYTES) {
        let _ = std::fs::rename(&path, dir.join(format!("{FILE_NAME}.1")));
    }
    let Ok(file) = OpenOptions::new().create(true).append(true).open(&path) else {
        return;
    };
    if log::set_boxed_logger(Box::new(FileLogger { file: Mutex::new(file) })).is_ok() {
        log::set_max_level(app_level().max(DEPENDENCIES_LEVEL));
    }
}
