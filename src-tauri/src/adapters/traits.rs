use async_trait::async_trait;

use crate::error::Result;
use crate::models::{
    ColumnInfo, DatabaseInfo, EditableInfo, IndexInfo, QueryOptions, QueryResult, RowEdit,
    SchemaInfo, StatementResult, TableInfo, DatabaseStructure
};

/// Core trait that all database adapters must implement
#[async_trait]
pub trait DatabaseAdapter: Send + Sync {
    // ─────────────────────────────────────────────────────────────────────
    // Query Execution
    // ─────────────────────────────────────────────────────────────────────

    async fn execute_query(
        &self,
        query: &str,
        options: QueryOptions,
    ) -> Result<QueryResult>;

    async fn apply_row_edits(
        &self,
        editable: &EditableInfo,
        edits: Vec<RowEdit>,
    ) -> Result<StatementResult>;

    async fn execute_statement(&self, statement: &str) -> Result<StatementResult>;

    async fn execute_transaction(
        &self,
        statements: Vec<String>,
    ) -> Result<Vec<StatementResult>>;

    // ─────────────────────────────────────────────────────────────────────
    // Metadata - Lazy Loading
    // ─────────────────────────────────────────────────────────────────────

    async fn list_databases(&self) -> Result<Vec<DatabaseInfo>>;

    async fn list_schemas(&self) -> Result<Vec<SchemaInfo>>;

    async fn list_tables(&self, schema: &str) -> Result<Vec<TableInfo>>;

    async fn list_columns(&self, schema: &str, table: &str) -> Result<Vec<ColumnInfo>>;

    async fn list_indexes(&self, schema: &str, table: &str) -> Result<Vec<IndexInfo>>;

    async fn list_schemas_with_tables(&self) -> Result<DatabaseStructure>; 

    // ─────────────────────────────────────────────────────────────────────
    // Connection
    // ─────────────────────────────────────────────────────────────────────

    async fn test_connection(&self) -> Result<()>;

    async fn cancel_query(&self, query_id: &str) -> Result<()>;

    fn pool_stats(&self) -> Option<PoolStats> {
        None
    }
}

/// Pool statistics for monitoring
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PoolStats {
    pub size: usize,
    pub available: usize,
    pub in_use: usize,
    pub waiting: usize,
}