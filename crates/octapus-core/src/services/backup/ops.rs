//! As operações de backup/restauração sobre o [`AppState`], as mesmas para o
//! app e para o terminal: achar as ferramentas, checar o servidor, montar o
//! plano e começar o job. Quem chama só escolhe o que fazer com os eventos.

use std::path::{Path, PathBuf};

use crate::error::{Error, Result};
use crate::models::{
    AdapterCapabilities, BackupPreview, BackupRequest, PgToolsDetection, PgToolsInstall, PgToolsStatus,
    QueryOptions, RestoreFormat, RestoreInspection, RestoreRequest, Server,
};
use crate::state::AppState;
use crate::storage::repositories::{servers, settings as settings_store};

use super::{
    connection_args, detect, detect_format, display_command, display_plan, dump_args, needs_timeout_filter,
    partial_path, restore_plan, ssl_env, status, suggested_file_name, summarize, version_from_server_setting,
    ConnectionArgs, JobInput, JobSpec, PlanInput, RestorePlan,
};

const MISSING_PG_RESTORE: &str =
    "Não há pg_restore na pasta das ferramentas do PostgreSQL — ele é preciso para ler arquivos .dump.";
const MISSING_PSQL: &str =
    "Não há psql na pasta das ferramentas do PostgreSQL — ele é preciso para rodar scripts .sql.";

/// Procura o `pg_dump`/`pg_restore`/`psql` (pasta das Preferências, `PATH`,
/// locais conhecidos), sem olhar para nenhum servidor.
pub async fn detect_tools(state: &AppState) -> Result<PgToolsDetection> {
    let settings = settings_store::load(&state.storage)?;
    Ok(detect(settings.pg_tools_dir.as_deref()).await)
}

/// As ferramentas frente ao servidor de `(server_id, database)`: a versão do
/// servidor e o que impede o backup/restore, se algo impedir.
pub async fn tools_status(state: &AppState, server_id: i64, database: &str) -> Result<PgToolsStatus> {
    postgres_only(&servers::get_by_id_meta(&state.storage, server_id)?)?;

    let (server_version, server_version_num) = server_version(state, server_id, database).await?;
    let detection = detect_tools(state).await?;
    Ok(status(detection, server_version, server_version_num))
}

/// A linha de comando (sem senha) e o nome sugerido, para mostrar antes de
/// rodar. Não decifra a senha nem procura as ferramentas.
pub fn preview_backup(state: &AppState, request: &BackupRequest) -> Result<BackupPreview> {
    let server = servers::get_by_id_meta(&state.storage, request.server_id)?;
    postgres_only(&server)?;

    let file_name = suggested_file_name(
        &request.database,
        &request.target,
        request.options.format,
        chrono::Local::now().naive_local(),
    );
    let connection = connection_args(&server, &request.database);
    let args = dump_args(&connection, &request.target, &request.options, &file_name).map_err(Error::External)?;
    Ok(BackupPreview {
        command: display_command("pg_dump", &args),
        file_name,
    })
}

/// Começa o `pg_dump` em segundo plano, gravando em `path` (via `.partial`).
/// Devolve o id do job; o andamento chega pelos eventos do `state.backups`.
pub async fn start_backup(state: &AppState, request: &BackupRequest, path: &str) -> Result<String> {
    // Decifra a senha: uma vez por job, fora de caminho quente
    let server = servers::get_by_id(&state.storage, request.server_id)?;
    postgres_only(&server)?;

    let install = selected_tools(state).await?;

    let partial = partial_path(path);
    let connection = connection_args(&server, &request.database);
    let args = dump_args(&connection, &request.target, &request.options, &partial).map_err(Error::External)?;

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

/// O arquivo a restaurar: formato (pelo conteúdo) e, num `.dump`, o resumo do
/// que tem dentro. Não toca em banco nenhum.
pub async fn inspect_restore(state: &AppState, path: String) -> Result<RestoreInspection> {
    let (format, size_bytes) = detect_format(Path::new(&path)).await.map_err(Error::External)?;

    let (summary, summary_error) = match format {
        RestoreFormat::Plain => (None, None),
        RestoreFormat::Custom => match selected_tools(state).await.map(|tools| tools.pg_restore) {
            Ok(Some(pg_restore)) => match summarize(Path::new(&pg_restore), Path::new(&path)).await {
                Ok(summary) => (Some(summary), None),
                Err(message) => (None, Some(message)),
            },
            Ok(None) => (None, Some(MISSING_PG_RESTORE.to_string())),
            Err(error) => (None, Some(error.to_string())),
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

/// A linha de comando da restauração (sem senha), para mostrar antes de
/// rodar — com o filtro do `SET transaction_timeout`, quando o servidor pede.
pub async fn preview_restore(state: &AppState, request: &RestoreRequest, path: &str) -> Result<String> {
    let server = servers::get_by_id_meta(&state.storage, request.server_id)?;
    postgres_only(&server)?;

    let plan = plan_restore(state, &server, request, path).await?;
    Ok(display_plan(&plan))
}

/// Começa a restauração de `path` no database do pedido (`pg_restore` ou
/// `psql`). `on_success` roda quando ela termina bem (no app, revalidar o
/// catálogo do database).
pub async fn start_restore(
    state: &AppState,
    request: &RestoreRequest,
    path: &str,
    on_success: Option<Box<dyn FnOnce() + Send>>,
) -> Result<String> {
    let server = servers::get_by_id(&state.storage, request.server_id)?;
    postgres_only(&server)?;

    let install = selected_tools(state).await?;
    let plan = plan_restore(state, &server, request, path).await?;
    let pg_restore = || install.pg_restore.clone().ok_or_else(|| missing(MISSING_PG_RESTORE));
    let program = match plan.tool {
        "pg_restore" => pg_restore()?,
        _ => install.psql.clone().ok_or_else(|| missing(MISSING_PSQL))?,
    };
    let input = match plan.input {
        None => None,
        Some(PlanInput::File(file)) => Some(JobInput::File(PathBuf::from(file))),
        Some(PlanInput::PgRestore(args)) => Some(JobInput::Process {
            tool: "pg_restore",
            program: PathBuf::from(pg_restore()?),
            args,
        }),
    };

    let connection = connection_args(&server, &request.database);
    Ok(state.backups.start(JobSpec {
        tool: plan.tool,
        program: PathBuf::from(program),
        args: plan.args,
        env: tool_env(&connection),
        output: None,
        tolerate_ignored_errors: plan.tolerate_ignored_errors,
        on_success,
        input,
    }))
}

/// O plano da restauração: com ferramentas 17+ num servidor anterior ao 17,
/// o SQL passa pelo filtro do `SET transaction_timeout` (ver `args.rs`).
async fn plan_restore(
    state: &AppState,
    server: &Server,
    request: &RestoreRequest,
    path: &str,
) -> Result<RestorePlan> {
    let (_, server_num) = server_version(state, request.server_id, &request.database).await?;
    let tools_num = selected_tools(state).await.map(|install| install.version_num).unwrap_or(0);
    let connection = connection_args(server, &request.database);
    restore_plan(
        &connection,
        request.format,
        &request.options,
        path,
        needs_timeout_filter(tools_num, server_num),
    )
    .map_err(Error::External)
}

/// `server_version` (sem o sufixo da distribuição) e `server_version_num` do
/// database, pela conexão em cache.
async fn server_version(state: &AppState, server_id: i64, database: &str) -> Result<(String, u32)> {
    let adapter = state.adapter(server_id, database).await?;
    let result = adapter
        .execute_query(
            "SELECT current_setting('server_version_num'), current_setting('server_version')",
            QueryOptions::default(),
        )
        .await?;
    let row = result.rows.first();
    let cell = |index: usize| row.and_then(|row| row.get(index)).and_then(|value| value.as_deref());
    Ok(version_from_server_setting(cell(0), cell(1)))
}

fn missing(message: &str) -> Error {
    Error::External(message.to_string())
}

/// A instalação em uso, ou o motivo de não haver uma.
async fn selected_tools(state: &AppState) -> Result<PgToolsInstall> {
    let detection = detect_tools(state).await?;
    match (detection.selected, detection.configured_error) {
        (_, Some(message)) => Err(Error::External(message)),
        (None, None) => Err(missing(
            "O pg_dump não foi encontrado. Instale as ferramentas do PostgreSQL ou escolha a pasta em Preferências › PostgreSQL.",
        )),
        (Some(install), None) => Ok(install),
    }
}

fn postgres_only(server: &Server) -> Result<()> {
    let supported = AdapterCapabilities::for_db_type(server.db_type).is_some_and(|caps| caps.supports_backup);
    if supported {
        Ok(())
    } else {
        Err(Error::UnsupportedDatabase(
            "Backup e restauração só existem para PostgreSQL.".into(),
        ))
    }
}

/// Ambiente do processo: a senha (fora do argv), o TLS do servidor e um
/// timeout de conexão — ninguém espera para sempre um host que não responde.
fn tool_env(connection: &ConnectionArgs) -> Vec<(String, String)> {
    let mut env: Vec<(String, String)> =
        ssl_env(connection).into_iter().map(|(key, value)| (key.to_string(), value)).collect();
    env.push(("PGCONNECT_TIMEOUT".to_string(), "10".to_string()));
    env.push(("PGAPPNAME".to_string(), "octapus_db".to_string()));
    if let Some(password) = &connection.password {
        env.push(("PGPASSWORD".to_string(), password.clone()));
    }
    env
}
