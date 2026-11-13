use std::sync::Arc;

use crate::adapters::{DatabaseAdapter, MessageSink};
use crate::error::Result;
use crate::models::{QueryOptions, QueryResult, StatementResult};

/// Query service - handles query execution logic
///
/// Currently a thin wrapper, but can be extended for:
/// - Query history tracking
/// - Query analysis/explain
/// - Query cancellation management
/// - Result caching
pub struct QueryService {
    // Future: query history storage, active queries tracking, etc.
}

impl QueryService {
    pub fn new() -> Self {
        Self {}
    }

    /// Execute a SELECT query with pagination
    ///
    /// `sink` recebe, em streaming, as mensagens que o banco emite fora do
    /// result set (RAISE NOTICE, erro detalhado, conclusão). `None` = sem log.
    pub async fn execute_query(
        &self,
        adapter: Arc<dyn DatabaseAdapter>,
        query: &str,
        options: QueryOptions,
        sink: Option<Arc<dyn MessageSink>>,
    ) -> Result<QueryResult> {
        // Future: log to query history, track active queries, etc.
        adapter.execute_query_with_messages(query, options, sink).await
    }

    /// Execute a statement (INSERT, UPDATE, DELETE, etc.)
    pub async fn execute_statement(
        &self,
        adapter: Arc<dyn DatabaseAdapter>,
        statement: &str,
    ) -> Result<StatementResult> {
        adapter.execute_statement(statement).await
    }

    /// Execute multiple statements in a transaction
    pub async fn execute_transaction(
        &self,
        adapter: Arc<dyn DatabaseAdapter>,
        statements: Vec<String>,
    ) -> Result<Vec<StatementResult>> {
        adapter.execute_transaction(statements).await
    }
}

impl Default for QueryService {
    fn default() -> Self {
        Self::new()
    }
}