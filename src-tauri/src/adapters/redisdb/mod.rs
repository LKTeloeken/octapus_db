mod browse;
mod command;
mod metadata;

use std::time::Instant;

use async_trait::async_trait;
use redis::aio::ConnectionManager;
use redis::{ConnectionAddr, ConnectionInfo, RedisConnectionInfo};
use tokio::sync::OnceCell;

use crate::adapters::DatabaseAdapter;
use crate::error::{Error, Result};
use crate::models::*;

pub struct RedisAdapter {
    client: redis::Client,
    /// Multiplexed auto-reconnecting connection, created lazily on first use
    /// (adapter construction is sync).
    manager: OnceCell<ConnectionManager>,
    db_index: i64,
}

impl RedisAdapter {
    pub fn new(server: &Server, database: &str) -> Result<Self> {
        let db_index: i64 = database.parse().map_err(|_| {
            Error::InvalidState(format!(
                "Redis database must be a numeric index, got '{database}'"
            ))
        })?;

        let client = if let Some(uri) = server.connection_uri.as_deref() {
            // URI carries host/credentials/TLS (redis:// or rediss://);
            // the selected database index still takes precedence
            let base = redis::Client::open(uri)
                .map_err(|e| Error::Connection(format!("Invalid Redis URI: {e}")))?;
            let mut info = base.get_connection_info().clone();
            info.redis.db = db_index;
            redis::Client::open(info).map_err(|e| Error::Connection(e.to_string()))?
        } else {
            let addr = if server.ssl_enabled {
                // Encrypt without certificate verification (self-signed servers)
                ConnectionAddr::TcpTls {
                    host: server.host.clone(),
                    port: server.port,
                    insecure: true,
                    tls_params: None,
                }
            } else {
                ConnectionAddr::Tcp(server.host.clone(), server.port)
            };

            let info = ConnectionInfo {
                addr,
                redis: RedisConnectionInfo {
                    db: db_index,
                    username: (!server.username.is_empty()).then(|| server.username.clone()),
                    password: (!server.password.is_empty()).then(|| server.password.clone()),
                    ..Default::default()
                },
            };

            redis::Client::open(info).map_err(|e| Error::Connection(e.to_string()))?
        };

        Ok(Self {
            client,
            manager: OnceCell::new(),
            db_index,
        })
    }

    async fn conn(&self) -> Result<ConnectionManager> {
        let manager = self
            .manager
            .get_or_try_init(|| async {
                ConnectionManager::new(self.client.clone()).await
            })
            .await
            .map_err(|e: redis::RedisError| Error::Connection(e.to_string()))?;

        Ok(manager.clone())
    }

    fn schema_name(&self) -> String {
        self.db_index.to_string()
    }
}

#[async_trait]
impl DatabaseAdapter for RedisAdapter {
    fn capabilities(&self) -> AdapterCapabilities {
        AdapterCapabilities::redis()
    }

    /// Free-form editor: one native Redis command per query,
    /// e.g. `GET user:1`, `HGETALL session:abc`, `SCAN 0 MATCH user:*`.
    async fn execute_query(
        &self,
        query: &str,
        _options: QueryOptions,
    ) -> Result<QueryResult> {
        let tokens = command::tokenize(query)?;
        let mut conn = self.conn().await?;

        let start = Instant::now();

        let mut cmd = redis::cmd(&tokens[0]);
        for arg in &tokens[1..] {
            cmd.arg(arg);
        }
        let value: redis::Value = cmd.query_async(&mut conn).await?;

        let execution_time_ms = start.elapsed().as_millis() as u64;

        let mut result = command::value_to_result(&tokens[0], value);
        result.execution_time_ms = execution_time_ms;
        Ok(result)
    }

    async fn fetch_table_data(&self, request: TableDataRequest) -> Result<QueryResult> {
        let mut conn = self.conn().await?;
        browse::fetch_table_data(&mut conn, request).await
    }

    async fn execute_statement(&self, statement: &str) -> Result<StatementResult> {
        let tokens = command::tokenize(statement)?;
        let mut conn = self.conn().await?;

        let start = Instant::now();

        let mut cmd = redis::cmd(&tokens[0]);
        for arg in &tokens[1..] {
            cmd.arg(arg);
        }
        let value: redis::Value = cmd.query_async(&mut conn).await?;

        // Integer replies (DEL, EXPIRE, ...) report how many keys were touched
        let affected = match value {
            redis::Value::Int(n) if n >= 0 => n as u64,
            _ => 0,
        };

        Ok(StatementResult {
            affected_rows: affected,
            execution_time_ms: start.elapsed().as_millis() as u64,
        })
    }

    async fn list_databases(&self) -> Result<Vec<DatabaseInfo>> {
        let mut conn = self.conn().await?;
        metadata::list_databases(&mut conn).await
    }

    async fn list_tables(&self, _schema: &str) -> Result<Vec<TableInfo>> {
        let mut conn = self.conn().await?;
        metadata::list_tables(&mut conn, &self.schema_name()).await
    }

    async fn list_columns(&self, _schema: &str, _table: &str) -> Result<Vec<ColumnInfo>> {
        Ok(metadata::list_columns())
    }

    async fn list_schemas_with_tables(&self) -> Result<DatabaseStructure> {
        let mut conn = self.conn().await?;
        metadata::list_schemas_with_tables(&mut conn, &self.schema_name()).await
    }

    async fn test_connection(&self) -> Result<()> {
        let mut conn = self.conn().await?;
        redis::cmd("PING").query_async::<String>(&mut conn).await?;
        Ok(())
    }
}
