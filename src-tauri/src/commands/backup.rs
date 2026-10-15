use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager, State};

use crate::models::{
    BackupPreview, BackupRequest, DatabaseType, PgToolsDetection, PgToolsInstall, PgToolsStatus,
    QueryOptions, RestoreFormat, RestoreInspection, RestoreRequest, Server,
};
use crate::services::backup::{self, JobInput, JobSpec};
use crate::state::AppState;
use crate::storage::repositories::{servers, settings as settings_store};

use super::connect_adapter;

/// Procura o `pg_dump`/`pg_restore`/`psql` (pasta das Preferências, `PATH`,
/// locais conhecidos), sem olhar para nenhum servidor — a tela de Preferências.
#[tauri::command]
pub async fn pg_tools_detect(state: State<'_, AppState>) -> Result<PgToolsDetection, String> {
    let settings = settings_store::load(&state.storage).map_err(|e| e.to_string())?;
    Ok(backup::detect(settings.pg_tools_dir.as_deref()).await)
}

/// As ferramentas frente ao servidor de `(server_id, database)`: a versão do
/// servidor e o que impede o backup/restore, se algo impedir.
#[tauri::command]
pub async fn pg_tools_status(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
) -> Result<PgToolsStatus, String> {
    postgres_only(&servers::get_by_id_meta(&state.storage, server_id).map_err(|e| e.to_string())?)?;

    let (server_version, server_version_num) = server_version(&state, server_id, &database).await?;

    let settings = settings_store::load(&state.storage).map_err(|e| e.to_string())?;
    let detection = backup::detect(settings.pg_tools_dir.as_deref()).await;
    Ok(backup::status(
        detection,
        server_version,
        server_version_num,
    ))
}

/// A linha de comando (sem senha) e o nome sugerido, para o diálogo mostrar
/// antes de rodar. Não decifra a senha nem procura as ferramentas.
#[tauri::command]
pub fn backup_preview(
    state: State<'_, AppState>,
    request: BackupRequest,
) -> Result<BackupPreview, String> {
    let server =
        servers::get_by_id_meta(&state.storage, request.server_id).map_err(|e| e.to_string())?;
    postgres_only(&server)?;

    let file_name = backup::suggested_file_name(
        &request.database,
        &request.target,
        request.options.format,
        chrono::Local::now().naive_local(),
    );
    let connection = backup::connection_args(&server, &request.database);
    let args = backup::dump_args(&connection, &request.target, &request.options, &file_name)?;
    Ok(BackupPreview {
        command: backup::display_command("pg_dump", &args),
        file_name,
    })
}

/// Começa o `pg_dump` em segundo plano, gravando em `path` (via `.partial`).
/// Devolve o id do job; o andamento chega pelo evento `backup-event`.
#[tauri::command]
pub async fn backup_start(
    state: State<'_, AppState>,
    request: BackupRequest,
    path: String,
) -> Result<String, String> {
    // Decifra a senha: uma vez por job, fora de caminho quente
    let server =
        servers::get_by_id(&state.storage, request.server_id).map_err(|e| e.to_string())?;
    postgres_only(&server)?;

    let install = selected_tools(&state).await?;

    let partial = backup::partial_path(&path);
    let connection = backup::connection_args(&server, &request.database);
    let args = backup::dump_args(&connection, &request.target, &request.options, &partial)?;

    Ok(state.backups.start(JobSpec {
        tool: "pg_dump",
        program: PathBuf::from(install.pg_dump),
        args,
        env: tool_env(&connection),
        output: Some((PathBuf::from(partial), PathBuf::from(path))),
        tolerate_ignored_errors: false,
        on_success: None,
        input: None,
    }))
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
) -> Result<RestoreInspection, String> {
    let (format, size_bytes) = backup::detect_format(Path::new(&path)).await?;

    let (summary, summary_error) = match format {
        RestoreFormat::Plain => (None, None),
        RestoreFormat::Custom => match selected_tools(&state).await.map(|t| t.pg_restore) {
            Ok(Some(pg_restore)) => {
                match backup::summarize(Path::new(&pg_restore), Path::new(&path)).await {
                    Ok(summary) => (Some(summary), None),
                    Err(message) => (None, Some(message)),
                }
            }
            Ok(None) => (None, Some(MISSING_PG_RESTORE.to_string())),
            Err(message) => (None, Some(message)),
        },
    };

    Ok(RestoreInspection {
        path,
        format,
        size_bytes,
        summary,
        summary_error,
    })
}

/// A linha de comando da restauração (sem senha), para o diálogo mostrar —
/// com o filtro do `SET transaction_timeout`, quando o servidor pede.
#[tauri::command]
pub async fn restore_preview(
    state: State<'_, AppState>,
    request: RestoreRequest,
    path: String,
) -> Result<String, String> {
    let server =
        servers::get_by_id_meta(&state.storage, request.server_id).map_err(|e| e.to_string())?;
    postgres_only(&server)?;

    let plan = plan_restore(&state, &server, &request, &path).await?;
    Ok(backup::display_plan(&plan))
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
) -> Result<String, String> {
    let server =
        servers::get_by_id(&state.storage, request.server_id).map_err(|e| e.to_string())?;
    postgres_only(&server)?;

    let install = selected_tools(&state).await?;
    let plan = plan_restore(&state, &server, &request, &path).await?;
    let pg_restore = || install.pg_restore.clone().ok_or(MISSING_PG_RESTORE);
    let program = match plan.tool {
        "pg_restore" => pg_restore()?,
        _ => install.psql.clone().ok_or(MISSING_PSQL)?,
    };
    let input = match plan.input {
        None => None,
        Some(backup::PlanInput::File(file)) => Some(JobInput::File(PathBuf::from(file))),
        Some(backup::PlanInput::PgRestore(args)) => Some(JobInput::Process {
            tool: "pg_restore",
            program: PathBuf::from(pg_restore()?),
            args,
        }),
    };

    let (server_id, database) = (request.server_id, request.database.clone());
    let refresh_catalog = Box::new(move || {
        let state = app.state::<AppState>();
        // Catálogo fechado se abre do zero na próxima vez; aberto, revalida
        if let Some(entry) = state.catalog.get(server_id, &database) {
            state.catalog.ensure_fresh(&entry, true);
        }
    });

    let connection = backup::connection_args(&server, &request.database);
    Ok(state.backups.start(JobSpec {
        tool: plan.tool,
        program: PathBuf::from(program),
        args: plan.args,
        env: tool_env(&connection),
        output: None,
        tolerate_ignored_errors: plan.tolerate_ignored_errors,
        on_success: Some(refresh_catalog),
        input,
    }))
}

/// O plano da restauração: com ferramentas 17+ num servidor anterior ao 17,
/// o SQL passa pelo filtro do `SET transaction_timeout` (ver `args.rs`).
async fn plan_restore(
    state: &State<'_, AppState>,
    server: &Server,
    request: &RestoreRequest,
    path: &str,
) -> Result<backup::RestorePlan, String> {
    let (_, server_num) = server_version(state, request.server_id, &request.database).await?;
    let tools_num = selected_tools(state)
        .await
        .map(|install| install.version_num)
        .unwrap_or(0);
    let connection = backup::connection_args(server, &request.database);
    backup::restore_plan(
        &connection,
        request.format,
        &request.options,
        path,
        backup::needs_timeout_filter(tools_num, server_num),
    )
}

/// `server_version` (sem o sufixo da distribuição) e `server_version_num` do
/// database, pela conexão em cache.
async fn server_version(
    state: &State<'_, AppState>,
    server_id: i64,
    database: &str,
) -> Result<(String, u32), String> {
    let adapter = connect_adapter(state, server_id, database).await?;
    let result = adapter
        .execute_query(
            "SELECT current_setting('server_version_num'), current_setting('server_version')",
            QueryOptions::default(),
        )
        .await
        .map_err(|e| e.to_string())?;
    let row = result.rows.first();
    let cell = |index: usize| {
        row.and_then(|row| row.get(index))
            .and_then(|v| v.as_deref())
    };
    Ok(backup::version_from_server_setting(cell(0), cell(1)))
}

const MISSING_PG_RESTORE: &str =
    "Não há pg_restore na pasta das ferramentas do PostgreSQL — ele é preciso para ler arquivos .dump.";
const MISSING_PSQL: &str =
    "Não há psql na pasta das ferramentas do PostgreSQL — ele é preciso para rodar scripts .sql.";

/// A instalação em uso, ou o motivo de não haver uma.
async fn selected_tools(state: &State<'_, AppState>) -> Result<PgToolsInstall, String> {
    let settings = settings_store::load(&state.storage).map_err(|e| e.to_string())?;
    let detection = backup::detect(settings.pg_tools_dir.as_deref()).await;
    match (detection.selected, detection.configured_error) {
        (_, Some(message)) => Err(message),
        (None, None) => Err(
            "O pg_dump não foi encontrado. Instale as ferramentas do PostgreSQL ou escolha a pasta em Preferências › PostgreSQL."
                .into(),
        ),
        (Some(install), None) => Ok(install),
    }
}

fn postgres_only(server: &Server) -> Result<(), String> {
    if server.db_type == DatabaseType::Postgres {
        Ok(())
    } else {
        Err("Backup e restauração só existem para PostgreSQL.".into())
    }
}

/// Ambiente do processo: a senha (fora do argv), o TLS do servidor e um
/// timeout de conexão — o app não espera para sempre um host que não responde.
fn tool_env(connection: &backup::ConnectionArgs) -> Vec<(String, String)> {
    let mut env = vec![
        ("PGSSLMODE".to_string(), connection.ssl_mode.to_string()),
        ("PGCONNECT_TIMEOUT".to_string(), "10".to_string()),
        ("PGAPPNAME".to_string(), "octapus_db".to_string()),
    ];
    if let Some(password) = &connection.password {
        env.push(("PGPASSWORD".to_string(), password.clone()));
    }
    env
}
