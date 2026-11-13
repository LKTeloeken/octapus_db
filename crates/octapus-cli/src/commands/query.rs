//! `octapus query`: uma consulta num servidor salvo.
//!
//! **Só leitura por padrão**, pelo mesmo caminho do assistente: o guard
//! (`assistant/guard`) e depois o executor só-leitura do adapter, que segura
//! sozinho (transação `READ ONLY` desfeita no fim, um statement só, tempo e
//! linhas com teto). É o que um script ou outro programa — uma IA rodando o
//! `octapus` pelo shell, por exemplo — consegue fazer.
//!
//! Escrever pede `--write`, e `--write` pede uma pessoa: só roda com um
//! terminal na entrada e no erro, depois de mostrar o comando e de a pessoa
//! digitar o nome do servidor. Não há flag que pule isso.

use std::io::Read;
use std::sync::Arc;

use octapus_core::adapters::{MessageSink, ReadOnlyOp};
use octapus_core::assistant::read_only;
use octapus_core::models::{QueryLanguage, QueryMessage, QueryMessageKind, QueryOptions, QueryResult};

use crate::cli::QueryArgs;
use crate::context::{ask, interactive, App, Connected};
use crate::error::{CliError, CliResult};
use crate::output::{Grid, QueryOutput};

/// Linhas por página do executor só-leitura (o teto dele).
const READ_PAGE: i64 = 1000;
/// Teto de linhas por chamada.
const MAX_LIMIT: i64 = 100_000;
/// Linhas do comando mostradas na confirmação de escrita.
const PREVIEW_LINES: usize = 30;

pub fn output_of(result: QueryResult) -> QueryOutput {
    QueryOutput {
        types: result.columns.iter().map(|column| column.type_name.clone()).collect(),
        grid: Grid {
            columns: result.columns.into_iter().map(|column| column.name).collect(),
            rows: result.rows,
        },
        has_more: result.has_more,
        total_count: result.total_count,
        elapsed_ms: result.execution_time_ms,
    }
}

/// O texto da consulta: o argumento, o arquivo ou a entrada padrão.
fn read_query(args: &QueryArgs) -> CliResult<String> {
    if let Some(path) = &args.file {
        return std::fs::read_to_string(path)
            .map_err(|error| CliError::Io(format!("não foi possível ler {}: {error}", path.display())));
    }
    match args.query.as_deref() {
        Some(text) if text != "-" => Ok(text.to_string()),
        _ => {
            use std::io::IsTerminal;
            if std::io::stdin().is_terminal() {
                return Err(CliError::Usage(
                    "falta a consulta: passe-a como argumento, com --file ou pela entrada padrão".into(),
                ));
            }
            let mut text = String::new();
            std::io::stdin().lock().read_to_string(&mut text)?;
            Ok(text)
        }
    }
}

pub async fn run(app: &App, args: &QueryArgs) -> CliResult<()> {
    // A recusa vem antes de ler qualquer coisa: sem terminal, não há escrita
    if args.write && !interactive() {
        log::warn!("escrita pedida sem terminal: recusada");
        return Err(CliError::WriteRefused(
            "--write só roda num terminal, com uma pessoa confirmando. Sem --write a consulta roda em modo só leitura."
                .into(),
        ));
    }

    let query = read_query(args)?;
    if query.trim().is_empty() {
        return Err(octapus_core::Error::InvalidQuery("Empty query".into()).into());
    }
    let connected = app.connect(&args.target).await?;

    let output = match args.write {
        true => write(&connected, &query, args).await?,
        false => read(&connected, &query, args).await?,
    };
    app.out.query(&output)
}

/// Leitura: guard + executor só-leitura, página a página até o `--limit`.
async fn read(connected: &Connected, query: &str, args: &QueryArgs) -> CliResult<QueryOutput> {
    let op = ReadOnlyOp::new(connected.capabilities.query_language, query);
    let limit = args.limit.clamp(1, MAX_LIMIT);
    let mut offset = args.offset.max(0);
    let mut output: Option<QueryOutput> = None;

    loop {
        let collected = output.as_ref().map_or(0, |output| output.grid.rows.len() as i64);
        let page = QueryOptions {
            limit: (limit - collected).min(READ_PAGE),
            offset,
            schema: args.schema.clone(),
            ..QueryOptions::default()
        };
        let result = read_only::execute(connected.adapter.as_ref(), connected.server.db_type, &op, page)
            .await
            .map_err(|error| hint_for(error, connected.capabilities.query_language))?;

        let fetched = result.rows.len() as i64;
        let has_more = result.has_more;
        match output.as_mut() {
            None => output = Some(output_of(result)),
            Some(output) => {
                output.grid.rows.extend(result.rows);
                output.elapsed_ms += result.execution_time_ms;
                output.has_more = has_more;
            }
        }
        offset += fetched;
        let collected = output.as_ref().map_or(0, |output| output.grid.rows.len() as i64);
        if !has_more || fetched == 0 || collected >= limit {
            break;
        }
    }
    Ok(output.unwrap_or_default())
}

/// No Mongo e no Redis a leitura protegida usa o formato declarativo do
/// assistente; o erro diz isso em vez de só "recusado".
fn hint_for(error: octapus_core::Error, language: QueryLanguage) -> CliError {
    let hint = match (&error, language) {
        (octapus_core::Error::GuardRejected(_), QueryLanguage::Mongo) => {
            " — no modo só leitura, o Mongo recebe uma operação em JSON: {\"collection\":\"…\",\"operation\":\"find\",\"filter\":{…}}"
        }
        (octapus_core::Error::GuardRejected(_), QueryLanguage::RedisPlan) => {
            " — no modo só leitura, o Redis recebe um plano em JSON: {\"steps\":[{\"id\":\"a\",\"command\":\"GET\",\"args\":[\"chave\"],\"foreach\":null}],\"output\":\"a\"}"
        }
        _ => return error.into(),
    };
    CliError::Core(octapus_core::Error::GuardRejected(format!(
        "{}{hint}",
        error.to_string().trim_start_matches("Guard rejected: ")
    )))
}

/// As mensagens do banco (NOTICE, linhas afetadas) no stderr, conforme chegam.
struct StderrSink;

impl MessageSink for StderrSink {
    fn push(&self, message: QueryMessage) {
        let label = match message.kind {
            QueryMessageKind::Error => "erro",
            QueryMessageKind::Warning => "aviso",
            QueryMessageKind::Notice | QueryMessageKind::Info => "nota",
            QueryMessageKind::Status => "",
        };
        match label {
            "" => eprintln!("{}", message.message),
            label => eprintln!("{label}: {}", message.message),
        }
    }
}

/// Escrita: o mesmo caminho do editor do app, depois de a pessoa confirmar.
async fn write(connected: &Connected, query: &str, args: &QueryArgs) -> CliResult<QueryOutput> {
    eprintln!(
        "Servidor: {} (id {}) · database: {}",
        connected.server.name, connected.server_id, connected.database
    );
    eprintln!("Vai executar, com permissão de escrita:\n");
    let lines: Vec<&str> = query.trim().lines().collect();
    for line in lines.iter().take(PREVIEW_LINES) {
        eprintln!("    {line}");
    }
    if lines.len() > PREVIEW_LINES {
        eprintln!("    … mais {} linha(s)", lines.len() - PREVIEW_LINES);
    }
    eprintln!();

    let answer = ask(&format!("Para confirmar, digite o nome do servidor ({}): ", connected.server.name))?;
    if answer != connected.server.name {
        return Err(CliError::WriteRefused("não confirmado: nada foi executado".into()));
    }
    log::info!(
        "server {} db {}: escrita confirmada no terminal",
        connected.server_id,
        connected.database
    );

    let options = QueryOptions {
        limit: args.limit.clamp(1, MAX_LIMIT),
        offset: args.offset.max(0),
        schema: args.schema.clone(),
        ..QueryOptions::default()
    };
    let sink: Arc<dyn MessageSink> = Arc::new(StderrSink);
    let result = connected.adapter.execute_query_with_messages(query, options, Some(sink)).await?;
    Ok(output_of(result))
}
