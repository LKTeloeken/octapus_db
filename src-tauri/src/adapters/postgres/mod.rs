mod browse;
mod pool;
mod executor;
mod metadata;
mod util;

use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;
use deadpool_postgres::Pool;

use crate::error::{Error, Result};
use crate::models::*;
use crate::adapters::{DatabaseAdapter, PoolStats};

/// Maps in-flight query ids to the backend PID of the connection running
/// them, so cancel_query can issue pg_cancel_backend from another connection.
type QueryRegistry = Mutex<HashMap<String, i32>>;

pub struct PostgresAdapter {
    pool: Pool,
    _database: String,
    active_queries: QueryRegistry,
}

impl PostgresAdapter {
    pub fn new(server: &Server, database: &str) -> Result<Self> {
        let pool = pool::create_pool(server, database)?;
        Ok(Self {
            pool,
            _database: database.to_string(),
            active_queries: Mutex::new(HashMap::new()),
        })
    }
}

#[async_trait]
impl DatabaseAdapter for PostgresAdapter {
    fn capabilities(&self) -> AdapterCapabilities {
        AdapterCapabilities::postgres()
    }

    async fn fetch_table_data(&self, request: TableDataRequest) -> Result<QueryResult> {
        browse::fetch_table_data(&self.pool, request).await
    }

    async fn execute_query(
        &self,
        query: &str,
        options: QueryOptions,
    ) -> Result<QueryResult> {
        executor::execute_query(&self.pool, query, options, &self.active_queries).await
    }

    async fn apply_row_edits(
        &self,
        editable: &EditableInfo,
        edits: Vec<RowEdit>,
    ) -> Result<StatementResult> {
        executor::apply_row_edits(&self.pool, editable, edits).await
    }

    async fn insert_rows(
        &self,
        editable: &EditableInfo,
        rows: Vec<RowInsert>,
    ) -> Result<StatementResult> {
        executor::insert_rows(&self.pool, editable, rows).await
    }

    async fn delete_rows(
        &self,
        editable: &EditableInfo,
        pk_values: Vec<Vec<Option<String>>>,
    ) -> Result<StatementResult> {
        executor::delete_rows(&self.pool, editable, pk_values).await
    }

    async fn execute_statement(&self, statement: &str) -> Result<StatementResult> {
        executor::execute_statement(&self.pool, statement).await
    }

    async fn execute_transaction(
        &self,
        statements: Vec<String>,
    ) -> Result<Vec<StatementResult>> {
        executor::execute_transaction(&self.pool, statements).await
    }

    async fn list_databases(&self) -> Result<Vec<DatabaseInfo>> {
        metadata::list_databases(&self.pool).await
    }

    async fn list_schemas(&self) -> Result<Vec<SchemaInfo>> {
        metadata::list_schemas(&self.pool).await
    }

    async fn list_tables(&self, schema: &str) -> Result<Vec<TableInfo>> {
        metadata::list_tables(&self.pool, schema).await
    }

    async fn list_columns(&self, schema: &str, table: &str) -> Result<Vec<ColumnInfo>> {
        metadata::list_columns(&self.pool, schema, table).await
    }

    async fn list_indexes(&self, schema: &str, table: &str) -> Result<Vec<IndexInfo>> {
        metadata::list_indexes(&self.pool, schema, table).await
    }

    async fn list_schemas_with_tables(&self) -> Result<DatabaseStructure> {
        metadata::list_schemas_with_tables(&self.pool).await
    }

    async fn test_connection(&self) -> Result<()> {
        let client = self.pool.get().await?;
        client.query_one("SELECT 1", &[]).await?;
        Ok(())
    }

    async fn cancel_query(&self, query_id: &str) -> Result<()> {
        let pid = self.active_queries.lock().unwrap().get(query_id).copied();

        // Unknown id means the query already finished (or never registered a
        // query_id); cancelling is then a benign no-op.
        let Some(pid) = pid else {
            return Ok(());
        };

        let client = self.pool.get().await?;
        let cancelled: bool = client
            .query_one("SELECT pg_cancel_backend($1)", &[&pid])
            .await?
            .get(0);

        if !cancelled {
            return Err(Error::Query(format!(
                "Could not cancel query {query_id}: backend {pid} no longer exists"
            )));
        }

        Ok(())
    }

    fn pool_stats(&self) -> Option<PoolStats> {
        let status = self.pool.status();
        Some(PoolStats {
            size: status.size,
            available: status.available,
            in_use: status.size - status.available,
            waiting: status.waiting,
        })
    }
}