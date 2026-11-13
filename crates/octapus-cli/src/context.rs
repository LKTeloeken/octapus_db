//! O que todo comando do `octapus` usa: os dados do app abertos
//! ([`AppState`]), a saída e a resolução de "servidor por id ou nome".

use std::io::{self, BufRead, IsTerminal};
use std::sync::Arc;

use octapus_core::adapters::DatabaseAdapter;
use octapus_core::models::{AdapterCapabilities, Server, ServerGroup};
use octapus_core::paths;
use octapus_core::state::AppState;
use octapus_core::storage::repositories::{server_groups, servers};
use octapus_core::Error;

use crate::cli::{Format, PasswordSource, Target};
use crate::error::{CliError, CliResult};
use crate::output::Output;

pub struct App {
    pub state: AppState,
    pub out: Output,
}

impl App {
    /// Abre os mesmos dados do app (`app.db`, `vault.key`, `catalog/`) e liga
    /// o log em arquivo.
    pub fn open(format: Option<Format>, no_color: bool) -> CliResult<Self> {
        if let Ok(dir) = paths::log_dir() {
            crate::logging::init(&dir);
        }
        let data_dir = paths::data_dir()?;
        let state = AppState::open(&data_dir)?;
        // O terminal não acompanha o progresso da sincronização do catálogo
        state.catalog.init(data_dir.join("catalog"), Arc::new(|_| {}));
        log::info!("octapus {} (terminal) iniciando", env!("CARGO_PKG_VERSION"));

        Ok(Self {
            state,
            out: Output::new(format, no_color),
        })
    }

    /// O servidor pelo id ou pelo nome (sem diferenciar maiúsculas), só com os
    /// metadados: a senha não é decifrada.
    pub fn server(&self, reference: &str) -> CliResult<Server> {
        let reference = reference.trim();
        if let Ok(id) = reference.parse::<i64>() {
            return Ok(servers::get_by_id_meta(&self.state.storage, id)?);
        }

        let mut matches: Vec<Server> = servers::get_all(&self.state.storage)?
            .into_iter()
            .filter(|server| server.name.eq_ignore_ascii_case(reference))
            .collect();
        match matches.len() {
            0 => Err(Error::NotFound(format!("Server \"{reference}\"")).into()),
            1 => Ok(matches.remove(0)),
            count => Err(CliError::Usage(format!(
                "{count} servidores se chamam \"{reference}\": use o id (octapus server list)"
            ))),
        }
    }

    /// O grupo pelo id ou pelo nome.
    pub fn group(&self, reference: &str) -> CliResult<ServerGroup> {
        let reference = reference.trim();
        let groups = server_groups::get_all(&self.state.storage)?;
        let found = match reference.parse::<i64>() {
            Ok(id) => groups.into_iter().find(|group| group.id == id),
            Err(_) => groups.into_iter().find(|group| group.name.eq_ignore_ascii_case(reference)),
        };
        found.ok_or_else(|| Error::NotFound(format!("Group \"{reference}\"")).into())
    }

    /// O servidor, o database (o pedido ou o padrão) e a conexão aberta.
    pub async fn connect(&self, target: &Target) -> CliResult<Connected> {
        let server = self.server(&target.server)?;
        let server_id = server_id(&server)?;
        let database = match target.database.as_deref().map(str::trim).filter(|name| !name.is_empty()) {
            Some(name) => name.to_string(),
            None => self.state.default_database(server_id)?,
        };
        let capabilities = AdapterCapabilities::for_db_type(server.db_type)
            .ok_or_else(|| Error::UnsupportedDatabase(format!("no adapter available for {:?}", server.db_type)))?;
        let adapter = self.state.adapter(server_id, &database).await?;

        Ok(Connected {
            server,
            server_id,
            database,
            capabilities,
            adapter,
        })
    }
}

/// Uma conexão aberta para um comando.
pub struct Connected {
    pub server: Server,
    pub server_id: i64,
    pub database: String,
    pub capabilities: AdapterCapabilities,
    pub adapter: Arc<dyn DatabaseAdapter>,
}

impl Connected {
    /// O schema de um pedido: o `--schema`, o `schema.` na frente do nome da
    /// tabela ou o padrão do banco (`public` no Postgres; nenhum nos outros).
    pub fn split_table(&self, table: &str, schema: Option<&str>) -> (String, String) {
        if let Some(schema) = schema {
            return (schema.to_string(), table.to_string());
        }
        if self.capabilities.has_schemas {
            if let Some((schema, table)) = table.split_once('.') {
                return (schema.to_string(), table.to_string());
            }
            return ("public".to_string(), table.to_string());
        }
        (String::new(), table.to_string())
    }
}

pub fn server_id(server: &Server) -> CliResult<i64> {
    server.id.ok_or_else(|| Error::InvalidState("Server has no ID".into()).into())
}

/// Há uma pessoa no terminal? (entrada e erro ligados a um terminal: a
/// pergunta sai no stderr e a resposta vem do stdin)
pub fn interactive() -> bool {
    io::stdin().is_terminal() && io::stderr().is_terminal()
}

/// Pergunta no terminal e devolve a linha digitada, sem as pontas.
pub fn ask(question: &str) -> CliResult<String> {
    eprint!("{question}");
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    Ok(line.trim().to_string())
}

/// A senha, de onde o pedido disser. `None` = nenhuma fonte pedida.
pub fn read_password(source: &PasswordSource, prompt: &str) -> CliResult<Option<String>> {
    if let Some(name) = &source.password_env {
        return std::env::var(name)
            .map(Some)
            .map_err(|_| CliError::Usage(format!("a variável {name} não está definida")));
    }
    if source.password_stdin {
        let mut line = String::new();
        io::stdin().lock().read_line(&mut line)?;
        return Ok(Some(line.trim_end_matches(['\n', '\r']).to_string()));
    }
    if source.password {
        return prompt_password(prompt).map(Some);
    }
    Ok(None)
}

/// Pergunta a senha sem eco; fora de um terminal não há a quem perguntar.
pub fn prompt_password(prompt: &str) -> CliResult<String> {
    if !interactive() {
        return Err(CliError::Usage(
            "sem terminal para perguntar a senha: use --password-env ou --password-stdin".into(),
        ));
    }
    rpassword::prompt_password(prompt).map_err(CliError::from)
}
