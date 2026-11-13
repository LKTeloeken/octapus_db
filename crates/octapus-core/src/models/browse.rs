use serde::Deserialize;

/// Request to browse a table's data server-side (pagination, sorting and
/// filtering happen in the adapter, not in a frontend-built query).
///
/// "Table" is interpreted per adapter: Postgres table/view, MongoDB collection,
/// Redis key-prefix group.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableDataRequest {
    /// Schema name; `None` for databases without schemas (MongoDB, Redis).
    #[cfg_attr(test, ts(optional = nullable))]
    pub schema: Option<String>,
    pub table: String,
    /// Postgres-only raw WHERE expression (without the `WHERE` keyword).
    /// Mongo/Redis reject a non-empty value.
    #[serde(default)]
    #[cfg_attr(test, ts(optional = nullable))]
    pub where_expr: Option<String>,
    #[serde(default)]
    #[cfg_attr(test, ts(as = "Option<Vec<SortSpec>>", optional))]
    pub sort: Vec<SortSpec>,
    #[serde(default = "default_limit")]
    #[cfg_attr(test, ts(as = "Option<i64>", optional))]
    pub limit: i64,
    #[serde(default)]
    #[cfg_attr(test, ts(as = "Option<i64>", optional))]
    pub offset: i64,
    #[serde(default)]
    #[cfg_attr(test, ts(as = "Option<bool>", optional))]
    pub count_total: bool,
    /// Ignora o `limit` e devolve todas as linhas que casam com o filtro. Usado
    /// pela exportação, que precisa do resultado inteiro e não de uma página.
    #[serde(default)]
    #[cfg_attr(test, ts(as = "Option<bool>", optional))]
    pub unlimited: bool,
}

fn default_limit() -> i64 {
    500
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SortSpec {
    pub column: String,
    #[serde(default)]
    pub direction: SortDirection,
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SortDirection {
    #[default]
    Asc,
    Desc,
}
