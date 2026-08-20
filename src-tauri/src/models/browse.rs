use serde::Deserialize;

/// Request to browse a table's data server-side (pagination, sorting and
/// filtering happen in the adapter, not in a frontend-built query).
///
/// "Table" is interpreted per adapter: Postgres table/view, MongoDB collection,
/// Redis key-prefix group.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableDataRequest {
    /// Schema name; `None` for databases without schemas (MongoDB, Redis).
    pub schema: Option<String>,
    pub table: String,
    /// Postgres-only raw WHERE expression (without the `WHERE` keyword).
    /// Mongo/Redis reject a non-empty value.
    #[serde(default)]
    pub where_expr: Option<String>,
    #[serde(default)]
    pub sort: Vec<SortSpec>,
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
    #[serde(default)]
    pub count_total: bool,
}

fn default_limit() -> i64 {
    500
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SortSpec {
    pub column: String,
    #[serde(default)]
    pub direction: SortDirection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SortDirection {
    #[default]
    Asc,
    Desc,
}
