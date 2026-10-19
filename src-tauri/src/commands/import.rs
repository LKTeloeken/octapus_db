//! Importar conexões de outros apps — ver services/connection_import.
//! Ler arquivos e o Keychain bloqueia (o Keychain pode esperar o usuário
//! responder ao aviso do macOS): roda fora da thread principal.

use tauri::State;

use crate::error::{Error, Result};
use crate::models::{
    ImportCommitRequest, ImportPreview, ImportResult, ImportSource, ImportSourceInfo,
};
use crate::services::connection_import;
use crate::state::AppState;

async fn blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> Result<T> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|e| Error::External(format!("Import task failed: {e}")))
}

/// Os apps desta máquina com conexões para importar.
#[tauri::command]
pub async fn import_detect_sources() -> Result<Vec<ImportSourceInfo>> {
    blocking(connection_import::detect).await
}

/// Lê uma origem e devolve a prévia, sem senhas. `path` escolhe o arquivo à
/// mão; `text` são as URLs coladas (origem `urls`).
#[tauri::command]
pub async fn import_preview(
    state: State<'_, AppState>,
    source: ImportSource,
    path: Option<String>,
    text: Option<String>,
) -> Result<ImportPreview> {
    let (path, raws) =
        blocking(move || connection_import::read_source(source, path, text)).await??;
    state.imports.preview(&state.storage, source, path, raws)
}

/// Importa as conexões escolhidas da prévia.
#[tauri::command]
pub async fn import_commit(
    state: State<'_, AppState>,
    request: ImportCommitRequest,
) -> Result<ImportResult> {
    let raws = state.imports.take(request.token, &request.keys)?;
    let include = request.include_passwords;
    let (raws, (passwords, denied)) = blocking(move || {
        let resolved = connection_import::resolve_passwords(&raws, include);
        (raws, resolved)
    })
    .await?;
    let result = connection_import::store(&state.storage, &raws, passwords, &request, denied);
    match &result {
        Ok(done) => log::info!(
            "importação: {} conexão(ões), {} sem senha, {} grupo(s) novo(s){}",
            done.imported,
            done.without_password,
            done.groups_created,
            if done.keychain_denied { "; Keychain negado" } else { "" }
        ),
        Err(error) => log::warn!("importação: falha ao gravar: {error}"),
    }
    result
}

/// Diálogo fechado sem importar: as senhas lidas saem da memória.
#[tauri::command]
pub fn import_discard(state: State<'_, AppState>) {
    state.imports.discard();
}
