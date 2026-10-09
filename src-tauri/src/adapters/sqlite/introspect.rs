//! Catálogo do SQLite (refactor do catálogo, Fase 5).
//!
//! Sem nível de schema: as tabelas e views do arquivo vão para o
//! [`FLAT_SCHEMA`]. O fingerprint é o `schema_version`, que o próprio SQLite
//! incrementa a cada mudança de estrutura. A conexão é só do catálogo (o
//! rusqlite é bloqueante e o editor não pode esperar por ela).

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use parking_lot::{Mutex, RwLock};
use rusqlite::{Connection, InterruptHandle, OptionalExtension};

use crate::catalog::{
    sync_flat, Catalog, CatalogSource, RelKind, SourceCanceller, SyncEvent, SyncReport,
    TableRecord, FLAT_SCHEMA,
};
use crate::error::{Error, Result};
use crate::models::Server;

use super::util::{db_err, quote_ident};

pub struct SqliteSource {
    conn: Arc<Mutex<Connection>>,
    interrupt: Arc<InterruptHandle>,
    /// `main`, `temp` ou um banco anexado (o `ATTACH` é da conexão do editor,
    /// então aqui só o `main` costuma ter conteúdo)
    database: String,
    version: String,
}

impl SqliteSource {
    pub async fn connect(server: &Server, database: &str) -> Result<Self> {
        let path = super::resolve_path(server)?;
        let conn = tokio::task::spawn_blocking(move || super::open(&path))
            .await
            .map_err(|e| Error::InvalidState(format!("SQLite task failed: {e}")))??;

        let version: String = conn
            .query_row("SELECT sqlite_version()", [], |row| row.get(0))
            .map_err(db_err)?;

        Ok(Self {
            interrupt: Arc::new(conn.get_interrupt_handle()),
            conn: Arc::new(Mutex::new(conn)),
            database: database.to_string(),
            version: format!("SQLite {version}"),
        })
    }

    /// Roda o trabalho bloqueante fora do executor do tokio. Uma interrupção
    /// pedida pelo canceller vira [`Error::Cancelled`].
    async fn with_conn<T, F>(&self, work: F) -> Result<T>
    where
        F: FnOnce(&Connection, &str) -> rusqlite::Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let conn = Arc::clone(&self.conn);
        let database = self.database.clone();
        let result = tokio::task::spawn_blocking(move || work(&conn.lock(), &database))
            .await
            .map_err(|e| Error::InvalidState(format!("SQLite task failed: {e}")))?;

        result.map_err(|error| match error.sqlite_error_code() {
            Some(rusqlite::ErrorCode::OperationInterrupted) => Error::Cancelled,
            _ => db_err(error),
        })
    }
}

/// `schema_version` e as relações do banco, numa leitura só.
fn read_tables(conn: &Connection, database: &str) -> rusqlite::Result<(i64, Vec<TableRecord>)> {
    let schema = quote_ident(database);
    let version: i64 = conn.query_row(&format!("PRAGMA {schema}.schema_version"), [], |row| row.get(0))?;

    // As internas (`sqlite_sequence`, `sqlite_stat1`…) ficam de fora, como na
    // listagem do adapter
    let mut stmt = conn.prepare(&format!(
        "SELECT name, type FROM {schema}.sqlite_schema \
         WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%' \
         ORDER BY name"
    ))?;
    let tables = stmt
        .query_map([], |row| {
            let kind: String = row.get(1)?;
            Ok(TableRecord {
                name: row.get(0)?,
                kind: if kind == "view" { RelKind::View } else { RelKind::Table },
                partition_of: None,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok((version, tables))
}

/// Páginas da tabela e dos índices dela (`dbstat`, compilado no SQLite do
/// app). View não tem páginas: `None`.
fn table_size(conn: &Connection, database: &str, table: &str) -> rusqlite::Result<Option<i64>> {
    let schema = quote_ident(database);
    conn.query_row(
        &format!(
            "SELECT sum(pgsize) FROM dbstat(?1) WHERE name IN (
                SELECT name FROM {schema}.sqlite_schema
                WHERE tbl_name = ?2 AND type IN ('table', 'index'))"
        ),
        [database, table],
        |row| row.get::<_, Option<i64>>(0),
    )
    .optional()
    .map(Option::flatten)
}

struct SqliteCanceller(Arc<InterruptHandle>);

#[async_trait]
impl SourceCanceller for SqliteCanceller {
    async fn cancel(&self) -> Result<()> {
        self.0.interrupt();
        Ok(())
    }
}

#[async_trait]
impl CatalogSource for SqliteSource {
    async fn sync(
        &mut self,
        catalog: &RwLock<Catalog>,
        on_event: &mut (dyn FnMut(SyncEvent) + Send),
    ) -> Result<SyncReport> {
        let started = Instant::now();
        let (version, tables) = self.with_conn(read_tables).await?;
        Ok(sync_flat(catalog, tables, version, started, on_event))
    }

    async fn load_schemas(&mut self, catalog: &RwLock<Catalog>, schemas: &[String]) -> Result<usize> {
        if !schemas.iter().any(|schema| schema == FLAT_SCHEMA) {
            return Ok(0);
        }
        let (version, tables) = self.with_conn(read_tables).await?;
        catalog.write().set_tables(FLAT_SCHEMA, tables, Some(version));
        Ok(1)
    }

    async fn relation_size(&mut self, _schema: &str, table: &str) -> Result<Option<i64>> {
        let table = table.to_string();
        // Sem `dbstat` (SQLite de fora), fica sem tamanho
        Ok(self
            .with_conn(move |conn, database| table_size(conn, database, &table))
            .await
            .unwrap_or(None))
    }

    fn canceller(&self) -> Box<dyn SourceCanceller> {
        Box::new(SqliteCanceller(Arc::clone(&self.interrupt)))
    }

    fn is_closed(&self) -> bool {
        false
    }

    fn server_version(&self) -> String {
        self.version.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{CatalogPath, LoadState};
    use crate::models::DatabaseType;

    fn server(path: &str) -> Server {
        Server {
            id: Some(1),
            name: "notas".into(),
            db_type: DatabaseType::Sqlite,
            host: String::new(),
            port: 0,
            username: String::new(),
            password: String::new(),
            default_database: None,
            ssl_enabled: false,
            connection_uri: Some(path.into()),
            scope_databases: None,
            scope_schemas: None,
            created_at: 0,
        }
    }

    /// Arquivo novo por teste (o adapter não cria banco)
    fn fresh_file(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("octapus_db_sqlite_catalog_{name}.db"));
        let _ = std::fs::remove_file(&path);
        path
    }

    fn names(catalog: &RwLock<Catalog>) -> Vec<String> {
        catalog
            .read()
            .children(&CatalogPath::Schema(FLAT_SCHEMA.into()), None, 0, 100)
            .unwrap()
            .items
            .into_iter()
            .map(|node| node.name)
            .collect()
    }

    #[tokio::test]
    async fn syncs_tables_and_views_and_follows_schema_changes() {
        let path = fresh_file("sync");
        let writer = Connection::open(&path).unwrap();
        writer
            .execute_batch(
                "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT);
                 CREATE INDEX idx_users_name ON users(name);
                 CREATE TABLE orders (id INTEGER PRIMARY KEY AUTOINCREMENT, user_id INTEGER);
                 CREATE VIEW active AS SELECT * FROM users;
                 INSERT INTO users (name) VALUES ('ana'), ('bia');",
            )
            .unwrap();

        let mut source = SqliteSource::connect(&server(path.to_str().unwrap()), "main").await.unwrap();
        assert!(source.server_version().starts_with("SQLite 3."));

        let catalog = RwLock::new(Catalog::new());
        let mut events = Vec::new();
        let report = source.sync(&catalog, &mut |event| events.push(event)).await.unwrap();
        assert_eq!(report.fetched, 1);
        assert_eq!(events.len(), 2);
        // `sqlite_sequence` (do AUTOINCREMENT) fica de fora
        assert_eq!(names(&catalog), vec!["active", "orders", "users"]);

        // Nada mudou: revalidar não recarrega
        let report = source.sync(&catalog, &mut |_| {}).await.unwrap();
        assert_eq!(report.fetched, 0);

        writer.execute_batch("CREATE TABLE invoices (id INTEGER)").unwrap();
        let report = source.sync(&catalog, &mut |_| {}).await.unwrap();
        assert_eq!(report.diff.changed, vec![FLAT_SCHEMA.to_string()]);
        assert_eq!(names(&catalog), vec!["active", "invoices", "orders", "users"]);
        assert_eq!(catalog.read().schema_state(FLAT_SCHEMA), Some(LoadState::Loaded));

        // Carga prioritária: só o schema sem nome existe
        assert_eq!(source.load_schemas(&catalog, &["outro".into()]).await.unwrap(), 0);
        assert_eq!(source.load_schemas(&catalog, &[FLAT_SCHEMA.into()]).await.unwrap(), 1);

        let size = source.relation_size(FLAT_SCHEMA, "users").await.unwrap();
        assert!(size.is_some_and(|bytes| bytes > 0));
        assert_eq!(source.relation_size(FLAT_SCHEMA, "active").await.unwrap(), None);
    }

    #[tokio::test]
    async fn interrupt_cancels_the_running_read() {
        let path = fresh_file("interrupt");
        Connection::open(&path)
            .unwrap()
            .execute_batch("CREATE TABLE t (id INTEGER)")
            .unwrap();
        let source = SqliteSource::connect(&server(path.to_str().unwrap()), "main").await.unwrap();
        let canceller = source.canceller();

        // Uma leitura sem fim, interrompida de outra task
        let running = source.with_conn(|conn, _| {
            conn.query_row(
                "WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM n) SELECT count(*) FROM n",
                [],
                |row| row.get::<_, i64>(0),
            )
        });
        let cancel = async {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            canceller.cancel().await.unwrap();
        };
        let (result, _) = tokio::join!(running, cancel);
        assert!(matches!(result, Err(Error::Cancelled)));
    }

    #[tokio::test]
    async fn missing_file_fails_to_connect() {
        let error = SqliteSource::connect(&server("/nao/existe.db"), "main").await.err().unwrap();
        assert!(error.to_string().contains("not found"));
    }
}
