use std::sync::Arc;

use crate::adapters::DatabaseAdapter;
use crate::error::Result;
use crate::models::{ColumnInfo, DatabaseInfo, IndexInfo, SchemaInfo, TableInfo};

/// Structure service - handles database metadata/structure
///
/// Currently a thin wrapper, but can be extended for:
/// - Structure caching
/// - Change detection
/// - Background refresh
pub struct StructureService {
    // Future: structure cache, refresh scheduling, etc.
}

impl StructureService {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn list_databases(
        &self,
        adapter: Arc<dyn DatabaseAdapter>,
    ) -> Result<Vec<DatabaseInfo>> {
        adapter.list_databases().await
    }

    pub async fn list_schemas(
        &self,
        adapter: Arc<dyn DatabaseAdapter>,
    ) -> Result<Vec<SchemaInfo>> {
        adapter.list_schemas().await
    }

    pub async fn list_tables(
        &self,
        adapter: Arc<dyn DatabaseAdapter>,
        schema: &str,
    ) -> Result<Vec<TableInfo>> {
        adapter.list_tables(schema).await
    }

    pub async fn list_columns(
        &self,
        adapter: Arc<dyn DatabaseAdapter>,
        schema: &str,
        table: &str,
    ) -> Result<Vec<ColumnInfo>> {
        adapter.list_columns(schema, table).await
    }

    pub async fn list_indexes(
        &self,
        adapter: Arc<dyn DatabaseAdapter>,
        schema: &str,
        table: &str,
    ) -> Result<Vec<IndexInfo>> {
        adapter.list_indexes(schema, table).await
    }
}

impl Default for StructureService {
    fn default() -> Self {
        Self::new()
    }
}