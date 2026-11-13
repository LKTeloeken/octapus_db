//! `octapus`: o Octapus no terminal. Abre os mesmos dados do app (`app.db`,
//! `vault.key`, `catalog/`) pelo núcleo (`octapus-core`), sem Tauri.

mod cli;
mod commands;
mod context;
mod error;
mod logging;
mod mcp;
mod output;
mod redact;
mod tui;

use std::io::IsTerminal;
use std::process::ExitCode;

use clap::{CommandFactory, Parser};
use serde_json::json;

use cli::{Cli, Command, Format, TuiArgs};
use context::App;
use error::{CliError, CliResult};

fn main() -> ExitCode {
    let cli = Cli::parse();
    let format = cli.format;

    let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("erro: não foi possível iniciar o runtime: {error}");
            return ExitCode::from(1);
        }
    };

    let result = runtime.block_on(run(cli));
    // Conexões abertas não seguram a saída do processo
    runtime.shutdown_background();

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            report(&error, format);
            ExitCode::from(error.exit_code())
        }
    }
}

/// O erro no stderr: objeto `{ code, message }` quando a saída é para
/// programa, uma linha quando é para gente.
fn report(error: &CliError, format: Option<Format>) {
    if matches!(error, CliError::BrokenPipe) {
        return;
    }
    log::warn!("comando falhou ({})", error.code());
    let for_program = match format {
        Some(Format::Json | Format::Ndjson) => true,
        Some(Format::Table | Format::Csv) => false,
        None => !std::io::stdout().is_terminal(),
    };
    if for_program {
        eprintln!("{}", json!({ "code": error.code(), "message": error.to_string() }));
    } else {
        eprintln!("erro [{}]: {error}", error.code());
    }
}

async fn run(cli: Cli) -> CliResult<()> {
    let command = match cli.command {
        Some(command) => command,
        // Sem subcomando: a TUI num terminal; fora dele, a ajuda
        None if std::io::stdin().is_terminal() && std::io::stdout().is_terminal() => Command::Tui(TuiArgs::default()),
        None => {
            Cli::command().print_help()?;
            return Err(CliError::Usage("falta o subcomando".into()));
        }
    };

    // Estes não tocam nos dados do app
    match &command {
        Command::Completions { shell } => {
            clap_complete::generate(*shell, &mut Cli::command(), "octapus", &mut std::io::stdout());
            return Ok(());
        }
        Command::Paths => return paths(&output::Output::new(cli.format, cli.no_color)),
        _ => {}
    }

    let app = App::open(cli.format, cli.no_color)?;
    match command {
        Command::Server(command) => commands::server::run(&app, command).await,
        Command::Group(command) => commands::group::run(&app, command),
        Command::Dbs(target) => commands::structure::databases(&app, &target).await,
        Command::Ls(args) => commands::structure::list(&app, &args).await,
        Command::Describe(args) => commands::structure::describe(&app, &args).await,
        Command::Table(args) => commands::table::run(&app, &args).await,
        Command::Query(args) => commands::query::run(&app, &args).await,
        Command::Backup(args) => commands::backup::run_backup(&app, &args).await,
        Command::Restore(args) => commands::backup::run_restore(&app, &args).await,
        Command::Tui(args) => tui::run(app, args).await,
        Command::Mcp(cli::McpCommand::Serve) => mcp::serve(app).await,
        Command::Completions { .. } | Command::Paths => Ok(()),
    }
}

fn paths(out: &output::Output) -> CliResult<()> {
    let show = |path: octapus_core::Result<std::path::PathBuf>| path.ok().map(|path| path.display().to_string());
    let data = show(octapus_core::paths::data_dir());
    let config = show(octapus_core::paths::config_dir());
    let logs = show(octapus_core::paths::log_dir());
    out.record(
        &[("dados", data.clone()), ("configuração", config.clone()), ("logs", logs.clone())],
        &json!({ "data": data, "config": config, "logs": logs }),
    )
}
