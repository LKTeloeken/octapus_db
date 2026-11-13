use tauri::{AppHandle, Manager, State};

use crate::error::Result;
use crate::models::{
    BackupPreview, BackupRequest, PgToolsDetection, PgToolsStatus, RestoreInspection, RestoreRequest,
};
use crate::services::backup;
use crate::state::AppState;

/// Procura o `pg_dump`/`pg_restore`/`psql` (pasta das Preferências, `PATH`,
/// locais conhecidos), sem olhar para nenhum servidor — a tela de Preferências.
#[tauri::command]
pub async fn pg_tools_detect(state: State<'_, AppState>) -> Result<PgToolsDetection> {
    backup::detect_tools(&state).await
}

/// As ferramentas frente ao servidor de `(server_id, database)`: a versão do
/// servidor e o que impede o backup/restore, se algo impedir.
#[tauri::command]
pub async fn pg_tools_status(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
) -> Result<PgToolsStatus> {
    backup::tools_status(&state, server_id, &database).await
}

/// A linha de comando (sem senha) e o nome sugerido, para o diálogo mostrar
/// antes de rodar. Não decifra a senha nem procura as ferramentas.
#[tauri::command]
pub fn backup_preview(state: State<'_, AppState>, request: BackupRequest) -> Result<BackupPreview> {
    backup::preview_backup(&state, &request)
}

/// Começa o `pg_dump` em segundo plano, gravando em `path` (via `.partial`).
/// Devolve o id do job; o andamento chega pelo evento `backup-event`.
#[tauri::command]
pub async fn backup_start(
    state: State<'_, AppState>,
    request: BackupRequest,
    path: String,
) -> Result<String> {
    backup::start_backup(&state, &request, &path).await
}

/// Cancela um job; o evento `cancelled` confirma (e o arquivo parcial some).
#[tauri::command]
pub fn backup_cancel(state: State<'_, AppState>, job_id: String) {
    state.backups.cancel(&job_id);
}

/// O arquivo a restaurar: formato (pelo conteúdo) e, num `.dump`, o resumo do
/// que tem dentro. Não toca em banco nenhum.
#[tauri::command]
pub async fn restore_inspect(
    state: State<'_, AppState>,
    path: String,
) -> Result<RestoreInspection> {
    backup::inspect_restore(&state, path).await
}

/// A linha de comando da restauração (sem senha), para o diálogo mostrar —
/// com o filtro do `SET transaction_timeout`, quando o servidor pede.
#[tauri::command]
pub async fn restore_preview(
    state: State<'_, AppState>,
    request: RestoreRequest,
    path: String,
) -> Result<String> {
    backup::preview_restore(&state, &request, &path).await
}

/// Começa a restauração de `path` no database do pedido (`pg_restore` ou
/// `psql`). Ao terminar, o catálogo do database é revalidado — as tabelas
/// novas aparecem na árvore pelo `catalog-event`.
#[tauri::command]
pub async fn restore_start(
    app: AppHandle,
    state: State<'_, AppState>,
    request: RestoreRequest,
    path: String,
) -> Result<String> {
    let (server_id, database) = (request.server_id, request.database.clone());
    let refresh_catalog = Box::new(move || {
        let state = app.state::<AppState>();
        // Catálogo fechado se abre do zero na próxima vez; aberto, revalida
        if let Some(entry) = state.catalog.get(server_id, &database) {
            state.catalog.ensure_fresh(&entry, true);
        }
    });
    backup::start_restore(&state, &request, &path, Some(refresh_catalog)).await
}
