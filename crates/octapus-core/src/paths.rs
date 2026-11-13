//! Onde ficam os dados do Octapus, sem depender do Tauri: o app e o terminal
//! abrem o mesmo `app.db`, o mesmo `vault.key` e a mesma pasta `catalog/`.
//!
//! A pasta de dados é a mesma que o Tauri dá ao app (`app_data_dir`): a pasta
//! de dados do usuário no SO mais o identificador do `tauri.conf.json`.

use std::path::PathBuf;

use crate::error::{Error, Result};

/// O `identifier` do `src-tauri/tauri.conf.json` (um teste do app confere).
pub const APP_IDENTIFIER: &str = "com.octapus-db.app";
/// Troca a pasta de dados (testes, outro perfil).
pub const DATA_DIR_ENV: &str = "OCTAPUS_DATA_DIR";
/// Troca a pasta de configuração do terminal.
pub const CONFIG_DIR_ENV: &str = "OCTAPUS_CONFIG_DIR";

fn from_env(name: &str) -> Option<PathBuf> {
    std::env::var_os(name).filter(|value| !value.is_empty()).map(PathBuf::from)
}

fn missing(what: &str) -> Error {
    Error::Storage(format!("Could not find the {what} directory of this user"))
}

/// `app.db`, `vault.key` e `catalog/` moram aqui.
pub fn data_dir() -> Result<PathBuf> {
    if let Some(dir) = from_env(DATA_DIR_ENV) {
        return Ok(dir);
    }
    Ok(dirs::data_dir().ok_or_else(|| missing("data"))?.join(APP_IDENTIFIER))
}

/// A pasta de logs do app (`app_log_dir` do Tauri): no macOS
/// `~/Library/Logs/<id>`; nos outros, `<dados locais>/<id>/logs`.
pub fn log_dir() -> Result<PathBuf> {
    if let Some(dir) = from_env(DATA_DIR_ENV) {
        return Ok(dir.join("logs"));
    }
    if cfg!(target_os = "macos") {
        return Ok(dirs::home_dir().ok_or_else(|| missing("home"))?.join("Library/Logs").join(APP_IDENTIFIER));
    }
    Ok(dirs::data_local_dir().ok_or_else(|| missing("data"))?.join(APP_IDENTIFIER).join("logs"))
}

/// Configuração do terminal (tema, atalhos): `$XDG_CONFIG_HOME/octapus` ou
/// `~/.config/octapus` no Unix (também no macOS, onde as ferramentas de
/// terminal ficam); no Windows, a pasta de configuração do usuário.
pub fn config_dir() -> Result<PathBuf> {
    if let Some(dir) = from_env(CONFIG_DIR_ENV) {
        return Ok(dir);
    }
    if cfg!(unix) {
        if let Some(dir) = from_env("XDG_CONFIG_HOME") {
            return Ok(dir.join("octapus"));
        }
        return Ok(dirs::home_dir().ok_or_else(|| missing("home"))?.join(".config/octapus"));
    }
    Ok(dirs::config_dir().ok_or_else(|| missing("config"))?.join("octapus"))
}
