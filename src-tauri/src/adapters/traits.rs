use std::sync::Arc;

use async_trait::async_trait;

use crate::adapters::MessageSink;
use crate::error::{Error, Result};
use crate::models::{
    AdapterCapabilities, ColumnInfo, DatabaseInfo, DatabaseStructure, EditableInfo, IndexInfo,
    QueryOptions, QueryResult, RowEdit, RowInsert, SchemaInfo, StatementResult, TableDataRequest,
    TableInfo,
};

/// Core trait that all database adapters must implement.
///
/// Non-relational adapters map their concepts onto the relational hierarchy
/// (database → schema → table → column); methods that don't apply have
/// default implementations so each adapter only overrides what it supports.
/// The frontend discovers what is available via [`DatabaseAdapter::capabilities`].
#[async_trait]
pub trait DatabaseAdapter: Send + Sync {
    // ─────────────────────────────────────────────────────────────────────
    // Capabilities
    // ─────────────────────────────────────────────────────────────────────

    // The get_capabilities command answers from AdapterCapabilities::for_db_type
    // (no connection needed); this method exists so future per-connection
    // capabilities (e.g. server version dependent) have a place to live.
    #[allow(dead_code)]
    fn capabilities(&self) -> AdapterCapabilities;

    // ─────────────────────────────────────────────────────────────────────
    // Query Execution
    // ─────────────────────────────────────────────────────────────────────

    /// Execute a free-form query in the adapter's native syntax
    /// (SQL for Postgres, shell-style commands for MongoDB, plain commands
    /// for Redis).
    async fn execute_query(
        &self,
        query: &str,
        options: QueryOptions,
    ) -> Result<QueryResult>;

    /// Igual ao [`DatabaseAdapter::execute_query`], mas com um canal para as
    /// mensagens que o banco emite fora do result set (no Postgres, os
    /// `RAISE NOTICE` e o erro detalhado) enquanto a query roda.
    ///
    /// O default delega e ignora o sink: um adapter sem esse conceito continua
    /// executando normalmente, só não alimenta o log.
    async fn execute_query_with_messages(
        &self,
        query: &str,
        options: QueryOptions,
        _sink: Option<Arc<dyn MessageSink>>,
    ) -> Result<QueryResult> {
        self.execute_query(query, options).await
    }

    /// Browse a table's data with server-side pagination, sorting and
    /// filtering. "Table" is adapter-defined (table/collection/key-prefix).
    async fn fetch_table_data(&self, request: TableDataRequest) -> Result<QueryResult>;

    async fn apply_row_edits(
        &self,
        _editable: &EditableInfo,
        _edits: Vec<RowEdit>,
    ) -> Result<StatementResult> {
        Err(Error::UnsupportedType(
            "Row editing is not supported for this database".into(),
        ))
    }

    async fn insert_rows(
        &self,
        _editable: &EditableInfo,
        _rows: Vec<RowInsert>,
    ) -> Result<StatementResult> {
        Err(Error::UnsupportedType(
            "Row insertion is not supported for this database".into(),
        ))
    }

    async fn delete_rows(
        &self,
        _editable: &EditableInfo,
        _pk_values: Vec<Vec<Option<String>>>,
    ) -> Result<StatementResult> {
        Err(Error::UnsupportedType(
            "Row deletion is not supported for this database".into(),
        ))
    }

    async fn execute_statement(&self, statement: &str) -> Result<StatementResult>;

    async fn execute_transaction(
        &self,
        _statements: Vec<String>,
    ) -> Result<Vec<StatementResult>> {
        Err(Error::UnsupportedType(
            "Transactions are not supported for this database".into(),
        ))
    }

    // ─────────────────────────────────────────────────────────────────────
    // Metadata - Lazy Loading
    // ─────────────────────────────────────────────────────────────────────

    async fn list_databases(&self) -> Result<Vec<DatabaseInfo>>;

    async fn list_schemas(&self) -> Result<Vec<SchemaInfo>> {
        Ok(vec![])
    }

    async fn list_tables(&self, schema: &str) -> Result<Vec<TableInfo>>;

    async fn list_columns(&self, schema: &str, table: &str) -> Result<Vec<ColumnInfo>>;

    async fn list_indexes(&self, _schema: &str, _table: &str) -> Result<Vec<IndexInfo>> {
        Ok(vec![])
    }

    async fn list_schemas_with_tables(&self) -> Result<DatabaseStructure>;

    // ─────────────────────────────────────────────────────────────────────
    // Connection
    // ─────────────────────────────────────────────────────────────────────

    async fn test_connection(&self) -> Result<()>;

    async fn cancel_query(&self, _query_id: &str) -> Result<()> {
        Err(Error::UnsupportedType(
            "Query cancellation is not supported for this database".into(),
        ))
    }

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
