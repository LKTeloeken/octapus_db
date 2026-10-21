//! Log do app: arquivo na pasta de logs do SO (`app_log_dir`, no macOS
//! `~/Library/Logs/com.octapus-db.app/`), com rotação, e o terminal em dev.
//!
//! O que vai para o log serve para diagnosticar um relato de bug, então tem
//! regra: **nunca** senha, URI de conexão, texto de query nem nomes de schema
//! ou tabela. Servidor pelo id e database pelo nome bastam; mensagens de erro
//! do banco entram como vieram.

use log::LevelFilter;
use tauri::plugin::TauriPlugin;
use tauri::Wry;
use tauri_plugin_log::{RotationStrategy, Target, TargetKind, TimezoneStrategy};

/// Nome do arquivo (o plugin acrescenta `.log`)
pub const LOG_FILE_NAME: &str = "octapus";
/// Tamanho de cada arquivo antes de girar
const MAX_FILE_BYTES: u128 = 5 * 1024 * 1024;
/// Arquivos antigos guardados além do atual
const KEEP_FILES: usize = 4;

/// O target dos logs do próprio app (o nome da lib).
pub const APP_TARGET: &str = "octapus_db_lib";
/// As dependências só a partir de `warn` — o `debug` do tokio-postgres, por
/// exemplo, traz o SQL.
pub const DEPENDENCIES_LEVEL: LevelFilter = LevelFilter::Warn;

/// Em dev o app loga em `debug`; em release, `info`.
pub fn app_level() -> LevelFilter {
    if cfg!(debug_assertions) {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    }
}

/// O arquivo de log com os níveis de [`DEPENDENCIES_LEVEL`] e [`app_level`].
pub fn plugin() -> TauriPlugin<Wry> {
    let app_level = app_level();

    let mut targets = vec![Target::new(TargetKind::LogDir {
        file_name: Some(LOG_FILE_NAME.into()),
    })];
    if cfg!(debug_assertions) {
        targets.push(Target::new(TargetKind::Stdout));
    }

    tauri_plugin_log::Builder::new()
        .clear_targets()
        .targets(targets)
        .level(DEPENDENCIES_LEVEL)
        .level_for(APP_TARGET, app_level)
        .max_file_size(MAX_FILE_BYTES)
        .rotation_strategy(RotationStrategy::KeepSome(KEEP_FILES))
        .timezone_strategy(TimezoneStrategy::UseLocal)
        .build()
}

/// Um panic vai para o log antes de seguir o caminho padrão (a mensagem no
/// stderr e o abort/unwind). Chamar depois que o plugin subiu.
pub fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "panic sem mensagem".into());
        log::error!("panic em {location}: {message}");
        default_hook(info);
    }));
}
