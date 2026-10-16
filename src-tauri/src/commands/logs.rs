use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

use crate::error::{Error, Result};
use crate::logging::LOG_FILE_NAME;

/// Abre a pasta de logs no gerenciador de arquivos, com o log atual
/// selecionado — para o usuário anexar a um relato de bug.
#[tauri::command]
pub fn open_log_dir(app: AppHandle) -> Result<()> {
    let dir = app
        .path()
        .app_log_dir()
        .map_err(|e| Error::External(format!("Pasta de logs indisponível: {e}")))?;
    let file = dir.join(format!("{LOG_FILE_NAME}.log"));

    let opened = if file.exists() {
        app.opener().reveal_item_in_dir(&file)
    } else {
        app.opener().open_path(dir.to_string_lossy(), None::<&str>)
    };
    opened.map_err(|e| Error::External(format!("Não foi possível abrir {}: {e}", dir.display())))
}
