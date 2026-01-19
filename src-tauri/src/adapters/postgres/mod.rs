mod pool;
mod executor;
mod metadata;
mod types;

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use deadpool_postgres::Pool;

use crate::error::Result;
use crate::models::*;
use crate::adapters::{DatabaseAdapter, PoolStats, PooledAdapter};

pub use pool::create_pool;

pub struct PostgresAdapter {
    pool: Pool,
    database: String,
}

impl PostgresAdapter {
    pub fn new(server: &Server, database: &str) -> Result<Self> {
        let pool = pool::create_pool(server, database)?;
        Ok(Self {
            pool,
            database: database.to_string(),
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
        executor::execute_query(&self.pool, query, options).await
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

    async fn test_connection(&self) -> Result<()> {
        let client = self.pool.get().await?;
        client.query_one("SELECT 1", &[]).await?;
        Ok(())
    }

    async fn cancel_query(&self, query_id: &str) -> Result<()> {
        // Implementation: parse query_id as PID, call pg_cancel_backend
        todo!("Implement query cancellation")
    }
}

impl PooledAdapter for PostgresAdapter {
    fn pool_stats(&self) -> PoolStats {
        let status = self.pool.status();
        PoolStats {
            size: status.size,
            available: status.available as usize,
            in_use: status.size - status.available as usize,
            waiting: status.waiting,
        }
    }
}