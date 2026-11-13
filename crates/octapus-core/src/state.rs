use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;
use rusqlite::Connection;

use crate::adapters::{self, create_catalog_source, has_catalog, DatabaseAdapter};
use crate::assistant::AssistantService;
use crate::error::{Error, Result};
use crate::models::{Server, ServerInput};
use crate::services::{connection_identity, CatalogEntry, SourceOpener};
use crate::storage::repositories::servers as server_store;
use crate::storage::{init_storage, vault};
use crate::services::backup::BackupService;
use crate::services::connection_import::ConnectionImportService;
use crate::services::{CatalogService, ConnectionService, QueryService, StructureService};

/// Um adapter parado há mais que isto leva um ping antes de ser usado.
const PING_AFTER_IDLE: Duration = Duration::from_secs(30);

/// O estado do Octapus num processo: o storage local, as conexões abertas e
/// os serviços. O app o entrega ao Tauri; o terminal o usa direto.
pub struct AppState {
    /// Local SQLite connection for app storage
    pub storage: Mutex<Connection>,

    /// Connection management service
    pub connections: ConnectionService,

    /// Query execution service
    pub queries: QueryService,

    /// Database structure service
    pub structure: StructureService,

    /// Catálogo de metadados (árvore, busca, autocomplete) — ver services/catalog.rs
    pub catalog: CatalogService,

    /// Jobs de backup/restauração (ferramentas do Postgres) — ver services/backup/
    pub backups: BackupService,

    /// Prévia da importação de conexões de outros apps — ver services/connection_import/
    pub imports: ConnectionImportService,

    /// Cliente da API do assistente de consulta — ver assistant/
    pub assistant: AssistantService,
}

impl AppState {
    pub fn new(storage_conn: Connection) -> Self {
        Self {
            storage: Mutex::new(storage_conn),
            connections: ConnectionService::new(),
            queries: QueryService::new(),
            structure: StructureService::new(),
            catalog: CatalogService::default(),
            backups: BackupService::default(),
            imports: ConnectionImportService::default(),
            assistant: AssistantService::from_env(),
        }
    }
}
impl AppState {
    /// Abre os dados do Octapus em `data_dir` (cria a pasta se preciso): a
    /// chave do vault e o `app.db`. Os eventos do catálogo e dos backups ficam
    /// por conta de quem chama (`catalog.init`, `backups.init`).
    pub fn open(data_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(data_dir)
            .map_err(|e| Error::Storage(format!("Failed to create the data directory: {e}")))?;
        vault::init(data_dir)?;
        let storage = init_storage(data_dir.join("app.db"))?;
        Ok(Self::new(storage))
    }

    /// Resolve the adapter for a connection, reusing the already-open pool when it
    /// exists. The server password is decrypted from the vault *only* on a real
    /// cache miss (when the pool has to be created), so the steady-state command
    /// path — browsing, listing structure, running queries against an open
    /// connection — never touches it.
    ///
    /// Um adapter em cache pode estar com a conexão morta (idle timeout do servidor,
    /// rede que caiu enquanto o app ficou aberto). Se ele ficou parado mais de
    /// [`PING_AFTER_IDLE`], fazemos um ping antes de devolvê-lo; se falhar,
    /// descartamos o cache e reconectamos — só nesse caminho de reconexão real a
    /// senha é decifrada. No Postgres o pool ainda testa, por conexão, as que
    /// ficaram ociosas (`pool.rs`).
    pub async fn adapter(&self, server_id: i64, database: &str) -> Result<Arc<dyn DatabaseAdapter>> {
        if let Some(adapter) = self.connections.get_live(server_id, database, PING_AFTER_IDLE).await {
            return Ok(adapter);
        }

        let server = server_store::get_by_id(&self.storage, server_id)?;
        self.connections.get_or_connect(&server, database).await
    }

    /// O database a usar quando nenhum foi pedido: o padrão do servidor ou o
    /// do tipo de banco. Só metadados, sem decifrar a senha.
    pub fn default_database(&self, server_id: i64) -> Result<String> {
        let server = server_store::get_by_id_meta(&self.storage, server_id)?;
        Ok(server
            .default_database
            .clone()
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| server.db_type.default_database_name().to_string()))
    }

    /// Catálogo do database, aberto na primeira vez (do disco, se houver) com uma
    /// revalidação em segundo plano. A senha só é decifrada aqui, quando a
    /// entrada ainda não existe.
    pub async fn catalog_entry(&self, server_id: i64, database: &str) -> Result<Arc<CatalogEntry>> {
        if let Some(entry) = self.catalog.get(server_id, database) {
            return Ok(entry);
        }

        let meta = server_store::get_by_id_meta(&self.storage, server_id)?;
        if !has_catalog(meta.db_type) {
            return Err(Error::UnsupportedDatabase(format!(
                "metadata catalog is not available for {:?}",
                meta.db_type
            )));
        }

        let server = server_store::get_by_id(&self.storage, server_id)?;
        let target = database.to_string();
        let opener: SourceOpener = Arc::new(move || {
            let (server, database) = (server.clone(), target.clone());
            Box::pin(async move { create_catalog_source(&server, &database).await })
        });

        let entry = self
            .catalog
            .open(server_id, database, connection_identity(&meta), opener)
            .await;
        self.catalog.ensure_fresh(&entry, false);
        Ok(entry)
    }

    /// Grava a edição do servidor. Só o que muda a conexão ou o que se enxerga
    /// do banco derruba as conexões e tira o catálogo da memória e do disco:
    /// renomear o servidor ou trocá-lo de grupo não refaz nada.
    pub fn update_server(&self, id: i64, input: ServerInput) -> Result<Server> {
        let current = server_store::get_by_id_meta(&self.storage, id)?;
        if connection_changed(&current, &input) {
            self.connections.disconnect_server(id);
            self.catalog.forget_server(id);
        }

        server_store::update(&self.storage, id, input)
    }

    /// Exclui o servidor, fechando antes as conexões e o catálogo dele.
    pub fn delete_server(&self, id: i64) -> Result<()> {
        self.connections.disconnect_server(id);
        self.catalog.forget_server(id);

        server_store::delete(&self.storage, id)
    }
}

/// A edição mexe em como o app conecta (endereço, credencial, TLS, fuso da
/// sessão) ou no escopo do catálogo? Senha informada conta como mudança: a
/// salva não é decifrada só para comparar.
fn connection_changed(current: &Server, input: &ServerInput) -> bool {
    fn blank_to_none(value: &Option<String>) -> Option<&str> {
        value.as_deref().filter(|text| !text.trim().is_empty())
    }

    let next = input.to_server(current.id);
    input.password.is_some()
        || next.db_type != current.db_type
        || next.host != current.host
        || next.port != current.port
        || next.username != current.username
        || next.default_database.as_deref() != blank_to_none(&current.default_database)
        || next.ssl_mode != current.ssl_mode
        || next.ssl_root_cert != current.ssl_root_cert
        || next.connection_uri.as_deref() != blank_to_none(&current.connection_uri)
        || next.display_timezone != current.display_timezone
        || blank_to_none(&input.scope_databases) != blank_to_none(&current.scope_databases)
        || blank_to_none(&input.scope_schemas) != blank_to_none(&current.scope_schemas)
}

/// Conecta ao database padrão do servidor (ou o do tipo) fora do pool, faz o
/// ping e solta a conexão, em até `timeout`.
pub async fn test_server(server: &Server, timeout: Duration) -> Result<()> {
    let database = server
        .default_database
        .clone()
        .unwrap_or_else(|| server.db_type.default_database_name().to_string());
    let test = async {
        let adapter = adapters::create_adapter(server, &database).await?;
        adapter.test_connection().await
    };
    match tokio::time::timeout(timeout, test).await {
        Ok(result) => result,
        Err(_) => Err(Error::ConnectionTimeout),
    }
}
