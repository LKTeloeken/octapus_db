use serde::{Deserialize, Serialize};
use super::structure::ForeignKeyTarget;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryResult {
    pub columns: Vec<QueryColumnInfo>,
    pub rows: Vec<Vec<Option<String>>>,
    pub row_count: usize,
    pub total_count: Option<i64>,
    pub has_more: bool,
    pub execution_time_ms: u64,
    pub editable_info: Option<EditableInfo>,
    pub query_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditableInfo {
    pub schema: String,
    pub table: String,
    pub primary_key_columns: Vec<String>,
    pub primary_key_column_indices: Vec<usize>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RowEdit {
    /// Primary key values in the same order as `EditableInfo::primary_key_columns`
    pub pk_values: Vec<Option<String>>,
    /// (column_name, new_value) pairs for each changed cell
    pub changes: Vec<(String, Option<String>)>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsertRowsInput {
    pub column_names: Vec<String>,
    pub rows: Vec<Vec<Option<String>>>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteRowsInput {
    pub pk_values_list: Vec<Vec<Option<String>>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryColumnInfo {
    pub name: String,
    pub type_name: String,
    pub type_oid: Option<u32>, // Postgres-specific, useful for editing
    pub foreign_key_target: Option<ForeignKeyTarget>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryOptions {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
    #[serde(default)]
    pub count_total: bool,
    #[serde(default)]
    pub unlimited: bool,
}

fn default_limit() -> i64 {
    500
}

impl Default for QueryOptions {
    fn default() -> Self {
        Self {
            limit: default_limit(),
            offset: 0,
            count_total: false,
            unlimited: false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatementResult {
    pub affected_rows: u64,
    pub execution_time_ms: u64,
}
