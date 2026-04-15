mod pool;
mod executor;
mod metadata;
mod types;

use async_trait::async_trait;
use deadpool_postgres::Pool;
use parking_lot::Mutex;
use std::sync::Arc;

use crate::error::Result;
use crate::models::*;
use crate::adapters::{DatabaseAdapter, PoolStats};

pub struct PostgresAdapter {
    pool: Pool,
    _database: String,
    active_query_pid: Arc<Mutex<Option<i32>>>,
}

impl PostgresAdapter {
    pub fn new(server: &Server, database: &str) -> Result<Self> {
        let pool = pool::create_pool(server, database)?;
        Ok(Self {
            pool,
            _database: database.to_string(),
            active_query_pid: Arc::new(Mutex::new(None)),
        })
    }
}

#[async_trait]
impl DatabaseAdapter for PostgresAdapter {
    async fn execute_query(
        &self,
        query: &str,
        options: QueryOptions,
    ) -> Result<QueryResult> {
        executor::execute_query(
            &self.pool,
            query,
            options,
            Arc::clone(&self.active_query_pid),
        )
        .await
    }

    async fn apply_row_edits(
        &self,
        editable: &EditableInfo,
        edits: Vec<RowEdit>,
    ) -> Result<StatementResult> {
        executor::apply_row_edits(&self.pool, editable, edits).await
    }

    async fn insert_table_rows(
        &self,
        editable: &EditableInfo,
        column_names: Vec<String>,
        rows: Vec<Vec<Option<String>>>,
    ) -> Result<StatementResult> {
        executor::insert_table_rows(&self.pool, editable, column_names, rows).await
    }

    async fn delete_table_rows(
        &self,
        editable: &EditableInfo,
        pk_values_list: Vec<Vec<Option<String>>>,
    ) -> Result<StatementResult> {
        executor::delete_table_rows(&self.pool, editable, pk_values_list).await
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
        executor::cancel_query(
            &self.pool,
            query_id,
            Arc::clone(&self.active_query_pid),
        )
        .await
    }

    fn pool_stats(&self) -> Option<PoolStats> {
        let status = self.pool.status();
        Some(PoolStats {
            size: status.size,
            available: status.available as usize,
            in_use: status.size - status.available as usize,
            waiting: status.waiting,
        })
    }
}
