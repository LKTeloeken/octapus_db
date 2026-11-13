//! O `octapus` de terminal visto do app — ver services/cli_install.

use crate::error::Result;
use crate::models::CliStatus;
use crate::services::cli_install;

/// O `octapus` já está instalado? Onde? Dá para chamá-lo pelo nome?
#[tauri::command]
pub fn cli_status() -> Result<CliStatus> {
    cli_install::status()
}

/// Baixa da release desta versão do app o `octapus` do sistema, confere o
/// SHA-256 e instala na pasta de binários do usuário.
#[tauri::command]
pub async fn cli_install() -> Result<CliStatus> {
    let result = cli_install::install(env!("CARGO_PKG_VERSION")).await;
    if let Err(error) = &result {
        log::warn!("octapus de terminal: instalação falhou ({})", error.code());
    }
    result
}
