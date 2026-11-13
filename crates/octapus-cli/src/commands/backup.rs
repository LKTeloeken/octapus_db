//! `octapus backup` e `octapus restore`: o `pg_dump`/`pg_restore`/`psql` do
//! app, pelo mesmo plano do núcleo (`services/backup`).
//!
//! O backup só lê o banco e pode rodar em script; ele não sobrescreve um
//! arquivo sem `--force`. A restauração **escreve** no banco: como o
//! `query --write`, só roda num terminal, depois de mostrar o que vai fazer e
//! de a pessoa digitar o nome do servidor. Não há flag que pule isso.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use octapus_core::models::{
    BackupEvent, BackupEventKind, BackupOptions, BackupRequest, BackupTarget, DumpContent, DumpFormat,
    RestoreFormat, RestoreOptions, RestoreRequest,
};
use octapus_core::services::backup;
use serde_json::json;
use tokio::sync::mpsc;

use crate::cli::{BackupArgs, Content, RestoreArgs};
use crate::context::{ask, interactive, App, Connected};
use crate::error::{CliError, CliResult};

pub async fn run_backup(app: &App, args: &BackupArgs) -> CliResult<()> {
    let connected = app.connect(&args.target).await?;
    let format = match args.sql || has_extension(args.output.as_deref(), "sql") {
        true => DumpFormat::Plain,
        false => DumpFormat::Custom,
    };
    let request = BackupRequest {
        server_id: connected.server_id,
        database: connected.database.clone(),
        target: target(&connected, args)?,
        options: BackupOptions {
            format,
            content: match args.content {
                Content::All => DumpContent::All,
                Content::Schema => DumpContent::SchemaOnly,
                Content::Data => DumpContent::DataOnly,
            },
            no_owner: args.no_owner,
            clean: args.clean,
            column_inserts: args.column_inserts,
        },
    };

    let preview = backup::preview_backup(&app.state, &request)?;
    let path = absolute(args.output.clone().unwrap_or_else(|| PathBuf::from(&preview.file_name)))?;
    if args.dry_run {
        return app.out.record(
            &[("comando", Some(preview.command.clone())), ("arquivo", Some(path.display().to_string()))],
            &json!({ "command": preview.command, "path": path }),
        );
    }
    if path.exists() && !args.force {
        return Err(CliError::Usage(format!(
            "{} já existe: escolha outro --output ou use --force",
            path.display()
        )));
    }

    let events = listen(app);
    let job = backup::start_backup(&app.state, &request, &path.to_string_lossy()).await?;
    let done = follow(app, &job, events, args.verbose).await?;
    app.out.done(
        &format!("backup gravado em {} ({} bytes, {} ms)", done.path, done.size_bytes, done.elapsed_ms),
        json!({ "path": done.path, "sizeBytes": done.size_bytes, "elapsedMs": done.elapsed_ms }),
    )
}

pub async fn run_restore(app: &App, args: &RestoreArgs) -> CliResult<()> {
    // A recusa vem antes de ler qualquer coisa: sem terminal, não há escrita
    if !args.dry_run && !interactive() {
        log::warn!("restauração pedida sem terminal: recusada");
        return Err(CliError::WriteRefused(
            "restore só roda num terminal, com uma pessoa confirmando. Use --dry-run para ver o que seria feito."
                .into(),
        ));
    }

    let path = absolute(args.file.clone())?;
    let path_text = path.to_string_lossy().into_owned();
    let inspection = backup::inspect_restore(&app.state, path_text.clone()).await?;
    let connected = app.connect(&args.target).await?;
    let request = RestoreRequest {
        server_id: connected.server_id,
        database: connected.database.clone(),
        format: inspection.format,
        options: RestoreOptions {
            no_owner: args.no_owner,
            clean: args.clean,
            single_transaction: args.single_transaction,
        },
    };
    let command = backup::preview_restore(&app.state, &request, &path_text).await?;

    if args.dry_run {
        let format = match inspection.format {
            RestoreFormat::Custom => "dump",
            RestoreFormat::Plain => "sql",
        };
        return app.out.record(
            &[
                ("comando", Some(command.clone())),
                ("arquivo", Some(path_text.clone())),
                ("formato", Some(format.to_string())),
                ("bytes", Some(inspection.size_bytes.to_string())),
            ],
            &json!({ "command": command, "file": inspection }),
        );
    }

    eprintln!(
        "Servidor: {} (id {}) · database: {}",
        connected.server.name, connected.server_id, connected.database
    );
    eprintln!("Arquivo: {path_text} ({} bytes)", inspection.size_bytes);
    if let Some(summary) = &inspection.summary {
        eprintln!(
            "Contém: {} schema(s), {} tabela(s), {} view(s){}",
            summary.schema_count,
            summary.table_count,
            summary.view_count,
            if summary.has_data { ", com dados" } else { ", só estrutura" }
        );
    }
    if let Some(reason) = &inspection.summary_error {
        eprintln!("aviso: {reason}");
    }
    eprintln!("Vai restaurar, escrevendo no banco:\n\n    {command}\n");
    let answer = ask(&format!("Para confirmar, digite o nome do servidor ({}): ", connected.server.name))?;
    if answer != connected.server.name {
        return Err(CliError::WriteRefused("não confirmado: nada foi executado".into()));
    }
    log::info!(
        "server {} db {}: restauração confirmada no terminal",
        connected.server_id,
        connected.database
    );

    let events = listen(app);
    let job = backup::start_restore(&app.state, &request, &path_text, None).await?;
    let done = follow(app, &job, events, args.verbose).await?;
    for warning in &done.warnings {
        eprintln!("aviso: {warning}");
    }
    app.out.done(
        &format!("restauração concluída em {} ms ({} aviso(s))", done.elapsed_ms, done.warnings.len()),
        json!({ "elapsedMs": done.elapsed_ms, "warnings": done.warnings }),
    )
}

/// O que entra no backup: o database, um schema ou uma tabela.
fn target(connected: &Connected, args: &BackupArgs) -> CliResult<BackupTarget> {
    match (&args.table, &args.schema) {
        (Some(table), schema) => {
            let (schema, table) = connected.split_table(table, schema.as_deref());
            Ok(BackupTarget::Table { schema, table })
        }
        (None, Some(schema)) => Ok(BackupTarget::Schema { schema: schema.clone() }),
        (None, None) => Ok(BackupTarget::Database),
    }
}

fn has_extension(path: Option<&Path>, extension: &str) -> bool {
    path.and_then(Path::extension).is_some_and(|found| found.eq_ignore_ascii_case(extension))
}

/// As ferramentas rodam em outro processo: o caminho vai inteiro.
fn absolute(path: PathBuf) -> CliResult<PathBuf> {
    Ok(std::path::absolute(path)?)
}

/// Liga os eventos dos jobs a um canal (antes de começar o job).
fn listen(app: &App) -> mpsc::UnboundedReceiver<BackupEvent> {
    let (sender, receiver) = mpsc::unbounded_channel();
    app.state.backups.init(Arc::new(move |event| {
        let _ = sender.send(event);
    }));
    receiver
}

struct Done {
    path: String,
    size_bytes: u64,
    elapsed_ms: u64,
    warnings: Vec<String>,
}

/// Acompanha o job até o fim. Ctrl+C cancela (o arquivo parcial some) e o
/// comando sai com o código de cancelado.
async fn follow(
    app: &App,
    job: &str,
    mut events: mpsc::UnboundedReceiver<BackupEvent>,
    verbose: bool,
) -> CliResult<Done> {
    let mut interrupted = Box::pin(tokio::signal::ctrl_c());
    let mut cancelling = false;
    loop {
        let event = tokio::select! {
            event = events.recv() => event,
            _ = &mut interrupted, if !cancelling => {
                cancelling = true;
                eprintln!("cancelando…");
                app.state.backups.cancel(job);
                continue;
            }
        };
        let Some(event) = event else {
            return Err(CliError::Io("o job terminou sem avisar".into()));
        };
        if event.job_id != job {
            continue;
        }
        match event.kind {
            BackupEventKind::Log { lines } => {
                if verbose {
                    for line in lines {
                        eprintln!("{line}");
                    }
                }
            }
            BackupEventKind::Done {
                path,
                size_bytes,
                elapsed_ms,
                warnings,
            } => {
                return Ok(Done {
                    path,
                    size_bytes,
                    elapsed_ms,
                    warnings,
                })
            }
            BackupEventKind::Error { message } => return Err(octapus_core::Error::External(message).into()),
            BackupEventKind::Cancelled => return Err(octapus_core::Error::Cancelled.into()),
        }
    }
}
