mod browse;
mod command;
mod executor;
mod metadata;
mod types;

use std::time::Duration;

use async_trait::async_trait;
use mongodb::bson::doc;
use mongodb::options::{ClientOptions, Credential, ServerAddress, Tls, TlsOptions};
use mongodb::{Client, Database};

use crate::adapters::DatabaseAdapter;
use crate::error::{Error, Result};
use crate::models::*;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

pub struct MongoAdapter {
    client: Client,
    database: String,
}

impl MongoAdapter {
    pub async fn new(server: &Server, database: &str) -> Result<Self> {
        let mut options = if let Some(uri) = server.connection_uri.as_deref() {
            // URI carries hosts/credentials/TLS (e.g. mongodb+srv:// for Atlas)
            ClientOptions::parse(uri)
                .await
                .map_err(|e| Error::Connection(format!("Invalid MongoDB URI: {e}")))?
        } else {
            let address = ServerAddress::Tcp {
                host: server.host.clone(),
                port: Some(server.port),
            };

            let mut options = ClientOptions::default();
            options.hosts = vec![address];

            if !server.username.is_empty() {
                options.credential = Some(
                    Credential::builder()
                        .username(server.username.clone())
                        .password(server.password.clone())
                        .build(),
                );
            }

            if server.ssl_enabled {
                options.tls = Some(Tls::Enabled(
                    TlsOptions::builder()
                        .allow_invalid_certificates(true)
                        .build(),
                ));
            }

            options
        };

        options.connect_timeout = Some(CONNECT_TIMEOUT);
        options.server_selection_timeout = Some(CONNECT_TIMEOUT);
        options.app_name = Some("octapus_db".to_string());

        let client = Client::with_options(options)
            .map_err(|e| Error::Connection(e.to_string()))?;

        Ok(Self {
            client,
            database: database.to_string(),
        })
    }

    fn db(&self) -> Database {
        self.client.database(&self.database)
    }
}

#[async_trait]
impl DatabaseAdapter for MongoAdapter {
    fn capabilities(&self) -> AdapterCapabilities {
        AdapterCapabilities::mongodb()
    }

    async fn execute_query(
        &self,
        query: &str,
        options: QueryOptions,
    ) -> Result<QueryResult> {
        executor::execute_query(&self.db(), &self.database, query, options).await
    }

    async fn fetch_table_data(&self, request: TableDataRequest) -> Result<QueryResult> {
        browse::fetch_table_data(&self.db(), &self.database, request).await
    }

    async fn apply_row_edits(
        &self,
        editable: &EditableInfo,
        edits: Vec<RowEdit>,
    ) -> Result<StatementResult> {
        executor::apply_row_edits(&self.db(), editable, edits).await
    }

    async fn insert_rows(
        &self,
        editable: &EditableInfo,
        rows: Vec<RowInsert>,
    ) -> Result<StatementResult> {
        executor::insert_rows(&self.db(), editable, rows).await
    }

    async fn delete_rows(
        &self,
        editable: &EditableInfo,
        pk_values: Vec<Vec<Option<String>>>,
    ) -> Result<StatementResult> {
        executor::delete_rows(&self.db(), editable, pk_values).await
    }

    async fn execute_statement(&self, statement: &str) -> Result<StatementResult> {
        executor::execute_statement(&self.db(), statement).await
    }

    async fn list_databases(&self) -> Result<Vec<DatabaseInfo>> {
        metadata::list_databases(&self.client).await
    }

    async fn list_tables(&self, _schema: &str) -> Result<Vec<TableInfo>> {
        metadata::list_tables(&self.db(), &self.database).await
    }

    async fn list_columns(&self, _schema: &str, table: &str) -> Result<Vec<ColumnInfo>> {
        metadata::list_columns(&self.db(), table).await
    }

    async fn list_indexes(&self, _schema: &str, table: &str) -> Result<Vec<IndexInfo>> {
        metadata::list_indexes(&self.db(), table).await
    }

    async fn list_schemas_with_tables(&self) -> Result<DatabaseStructure> {
        metadata::list_schemas_with_tables(&self.db(), &self.database).await
    }

    async fn test_connection(&self) -> Result<()> {
        self.db().run_command(doc! { "ping": 1 }).await?;
        Ok(())
    }
}
