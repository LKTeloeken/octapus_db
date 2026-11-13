//! `octapus server …`: as conexões salvas. A senha entra (por pergunta sem
//! eco, variável de ambiente, entrada padrão ou dentro da URL) e nunca sai:
//! nenhuma saída a devolve, e a URI de conexão sai sem a credencial.

use std::time::Duration;

use octapus_core::models::{DatabaseType, Server, ServerInput, SslMode};
use octapus_core::services::connection_import::server_input_from_url;
use octapus_core::state::test_server;
use octapus_core::storage::repositories::{server_groups, servers};
use serde_json::{json, Value};

use crate::cli::{DbType, PasswordSource, ServerAddArgs, ServerCommand, ServerEditArgs};
use crate::context::{ask, interactive, prompt_password, read_password, server_id, App};
use crate::error::{CliError, CliResult};
use crate::output::Grid;
use crate::redact;

/// Teto do `server test`: um host que não responde não segura o comando.
const TEST_TIMEOUT: Duration = Duration::from_secs(20);

/// O servidor como objeto, do jeito que o app o vê (camelCase, sem senha),
/// com a URI de conexão sem a credencial.
pub fn value(server: &Server) -> Value {
    let mut value = serde_json::to_value(server).unwrap_or(Value::Null);
    if let (Some(object), Some(uri)) = (value.as_object_mut(), server.connection_uri.as_deref()) {
        object.insert("connectionUri".into(), Value::String(redact::uri(uri)));
    }
    value
}

fn type_name(db_type: DatabaseType) -> String {
    serde_json::to_value(db_type)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_default()
}

fn non_empty(value: &Option<String>) -> Option<String> {
    value.clone().filter(|text| !text.trim().is_empty())
}

pub async fn run(app: &App, command: ServerCommand) -> CliResult<()> {
    match command {
        ServerCommand::List => list(app),
        ServerCommand::Show { server } => show(app, &app.server(&server)?),
        ServerCommand::Add(args) => add(app, args),
        ServerCommand::Edit(args) => edit(app, args),
        ServerCommand::Rm { server, yes } => remove(app, &server, yes),
        ServerCommand::Test { server } => test(app, &server).await,
        ServerCommand::SetPassword { server, password } => set_password(app, &server, &password),
    }
}

fn list(app: &App) -> CliResult<()> {
    let servers = servers::get_all(&app.state.storage)?;
    let groups = server_groups::get_all(&app.state.storage)?;
    let group_name = |id: Option<i64>| id.and_then(|id| groups.iter().find(|group| group.id == id)).map(|group| group.name.clone());

    let mut grid = Grid::new(["ID", "NOME", "TIPO", "HOST", "PORTA", "USUÁRIO", "DATABASE", "GRUPO"]);
    for server in &servers {
        grid.push([
            server.id.map(|id| id.to_string()),
            Some(server.name.clone()),
            Some(type_name(server.db_type)),
            non_empty(&Some(server.host.clone())),
            (server.port != 0).then(|| server.port.to_string()),
            non_empty(&Some(server.username.clone())),
            non_empty(&server.default_database),
            group_name(server.group_id),
        ]);
    }
    app.out.list(&grid, &servers.iter().map(value).collect::<Vec<_>>())
}

fn show(app: &App, server: &Server) -> CliResult<()> {
    let fields = [
        ("id", server.id.map(|id| id.to_string())),
        ("nome", Some(server.name.clone())),
        ("tipo", Some(type_name(server.db_type))),
        ("host", non_empty(&Some(server.host.clone()))),
        ("porta", (server.port != 0).then(|| server.port.to_string())),
        ("usuário", non_empty(&Some(server.username.clone()))),
        ("database", non_empty(&server.default_database)),
        ("ssl", Some(server.ssl_mode.as_str().to_string())),
        ("uri", server.connection_uri.as_deref().map(redact::uri)),
        ("grupo", server.group_id.map(|id| id.to_string())),
    ];
    app.out.record(&fields, &value(server))
}

fn parse_ssl_mode(raw: &str) -> CliResult<SslMode> {
    SslMode::parse(raw.trim()).ok_or_else(|| {
        CliError::Usage(format!(
            "modo de SSL \"{raw}\" inválido: use disable, prefer, require, verify-ca ou verify-full"
        ))
    })
}

fn group_id(app: &App, name: &str) -> CliResult<Option<i64>> {
    if name.trim().is_empty() {
        return Ok(None);
    }
    Ok(Some(app.group(name)?.id))
}

/// O cadastro a partir dos campos do pedido, ainda sem a senha de fora da
/// URL (quem chama decide de onde ela vem).
pub fn input_from_args(app: &App, args: &ServerAddArgs) -> CliResult<ServerInput> {
    let mut input = match (&args.url, args.db_type) {
        (Some(url), _) => server_input_from_url(url).ok_or_else(|| {
            CliError::Usage(
                "URL de conexão não reconhecida: use postgres://, mongodb://, redis:// ou o caminho de um arquivo SQLite".into(),
            )
        })?,
        (None, Some(DbType::Sqlite)) => {
            return Err(CliError::Usage("para SQLite, passe o caminho do arquivo: octapus server add /caminho/app.db".into()))
        }
        (None, Some(db_type)) => {
            let db_type = match db_type {
                DbType::Postgres => DatabaseType::Postgres,
                DbType::Mongodb => DatabaseType::Mongodb,
                DbType::Redis => DatabaseType::Redis,
                DbType::Sqlite => DatabaseType::Sqlite,
            };
            let host = args
                .host
                .clone()
                .ok_or_else(|| CliError::Usage("falta --host (ou passe uma URL de conexão)".into()))?;
            ServerInput {
                name: host.clone(),
                db_type,
                host,
                port: db_type.default_port(),
                username: String::new(),
                password: None,
                default_database: None,
                ssl_mode: None,
                ssl_root_cert: None,
                connection_uri: None,
                scope_databases: None,
                scope_schemas: None,
                group_id: None,
                assistant_sample_values: false,
                display_timezone: None,
            }
        }
        (None, None) => {
            return Err(CliError::Usage(
                "passe uma URL de conexão ou --type com --host (octapus server add --help)".into(),
            ))
        }
    };

    if let Some(name) = &args.name {
        input.name = name.clone();
    }
    if let Some(host) = &args.host {
        input.host = host.clone();
    }
    if let Some(port) = args.port {
        input.port = port;
    }
    if let Some(user) = &args.user {
        input.username = user.clone();
    }
    if let Some(database) = &args.database {
        input.default_database = Some(database.clone());
    }
    if let Some(mode) = &args.ssl_mode {
        input.ssl_mode = Some(parse_ssl_mode(mode)?);
    }
    if let Some(group) = &args.group {
        input.group_id = group_id(app, group)?;
    }
    Ok(input)
}

fn add(app: &App, args: ServerAddArgs) -> CliResult<()> {
    let mut input = input_from_args(app, &args)?;
    if let Some(password) = read_password(&args.password, "Senha: ")? {
        input.password = Some(password);
    }

    let server = servers::create(&app.state.storage, input)?;
    log::info!("server {}: criado pelo terminal ({:?})", server.id.unwrap_or_default(), server.db_type);
    app.out.done(
        &format!("servidor {} criado (id {})", server.name, server.id.unwrap_or_default()),
        json!({ "server": value(&server) }),
    )
}

fn edit(app: &App, args: ServerEditArgs) -> CliResult<()> {
    let current = app.server(&args.server)?;
    let id = server_id(&current)?;

    let group = match &args.group {
        Some(group) => group_id(app, group)?,
        None => current.group_id,
    };
    let ssl_mode = match &args.ssl_mode {
        Some(mode) => parse_ssl_mode(mode)?,
        None => current.ssl_mode,
    };
    // `password: None` mantém a senha salva
    let input = ServerInput {
        name: args.name.unwrap_or(current.name),
        db_type: current.db_type,
        host: args.host.unwrap_or(current.host),
        port: args.port.unwrap_or(current.port),
        username: args.user.unwrap_or(current.username),
        password: None,
        default_database: args.database.or(current.default_database),
        ssl_mode: Some(ssl_mode),
        ssl_root_cert: current.ssl_root_cert,
        connection_uri: current.connection_uri,
        scope_databases: current.scope_databases,
        scope_schemas: current.scope_schemas,
        group_id: group,
        assistant_sample_values: current.assistant_sample_values,
        display_timezone: current.display_timezone,
    };

    let server = app.state.update_server(id, input)?;
    log::info!("server {id}: editado pelo terminal");
    app.out.done(&format!("servidor {} atualizado", server.name), json!({ "server": value(&server) }))
}

fn remove(app: &App, reference: &str, yes: bool) -> CliResult<()> {
    let server = app.server(reference)?;
    let id = server_id(&server)?;

    if !yes {
        if !interactive() {
            return Err(CliError::Usage("fora de um terminal, confirme com --yes".into()));
        }
        let answer = ask(&format!("Excluir o servidor \"{}\" (id {id})? [s/N] ", server.name))?;
        if !matches!(answer.to_lowercase().as_str(), "s" | "sim" | "y" | "yes") {
            return Err(CliError::Usage("nada foi excluído".into()));
        }
    }

    app.state.delete_server(id)?;
    log::info!("server {id}: excluído pelo terminal");
    app.out.done(&format!("servidor {} excluído", server.name), json!({ "id": id }))
}

async fn test(app: &App, reference: &str) -> CliResult<()> {
    let meta = app.server(reference)?;
    let id = server_id(&meta)?;
    // Decifra a senha: uma vez, só para este teste
    let server = servers::get_by_id(&app.state.storage, id)?;

    let started = std::time::Instant::now();
    let result = test_server(&server, TEST_TIMEOUT).await;
    let elapsed = started.elapsed().as_millis() as u64;
    match &result {
        Ok(()) => log::info!("server {id}: teste de conexão ok em {elapsed} ms"),
        Err(error) => log::info!("server {id}: teste de conexão falhou ({})", error.code()),
    }
    result?;
    app.out.done(&format!("conexão ok ({elapsed} ms)"), json!({ "id": id, "elapsedMs": elapsed }))
}

fn set_password(app: &App, reference: &str, source: &PasswordSource) -> CliResult<()> {
    let current = app.server(reference)?;
    let id = server_id(&current)?;
    let password = match read_password(source, "Nova senha: ")? {
        Some(password) => password,
        None => prompt_password("Nova senha: ")?,
    };

    let input = ServerInput {
        name: current.name.clone(),
        db_type: current.db_type,
        host: current.host,
        port: current.port,
        username: current.username,
        password: Some(password),
        default_database: current.default_database,
        ssl_mode: Some(current.ssl_mode),
        ssl_root_cert: current.ssl_root_cert,
        connection_uri: current.connection_uri,
        scope_databases: current.scope_databases,
        scope_schemas: current.scope_schemas,
        group_id: current.group_id,
        assistant_sample_values: current.assistant_sample_values,
        display_timezone: current.display_timezone,
    };
    app.state.update_server(id, input)?;
    log::info!("server {id}: senha trocada pelo terminal");
    app.out.done(&format!("senha do servidor {} atualizada", current.name), json!({ "id": id }))
}
