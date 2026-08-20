use serde::{Deserialize, Serialize};

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
pub struct RowInsert {
    /// (column_name, value) pairs for the filled columns only. Columns left
    /// out let the database apply its own default (serial, default, `_id`).
    pub values: Vec<(String, Option<String>)>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryColumnInfo {
    pub name: String,
    pub type_name: String,
    /// Optional driver-specific type metadata (Postgres OID); `None` for
    /// adapters without a native type id (MongoDB, Redis).
    pub type_oid: Option<u32>,
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
    /// Frontend-generated id used to target this execution from the
    /// `cancel_query` command; without it the query cannot be cancelled.
    #[serde(default)]
    pub query_id: Option<String>,
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
            query_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatementResult {
    pub affected_rows: u64,
    pub execution_time_ms: u64,
}
/// Categoria da mensagem, usada pelo front para escolher o estilo da linha.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum QueryMessageKind {
    Notice,
    Warning,
    Error,
    Info,
    /// Linha sintética de conclusão emitida pelo próprio app, não pelo banco.
    Status,
}

/// Tudo que o banco emite fora do result set durante uma execução: os
/// `RAISE NOTICE/WARNING/INFO`, o erro detalhado (com a pilha do PL/pgSQL em
/// `context`) e a linha de conclusão. Chega ao front em streaming, não dentro
/// do `QueryResult`.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryMessage {
    pub kind: QueryMessageKind,
    /// Severidade crua do servidor (NOTICE, WARNING, ERROR, INFO, LOG, DEBUG).
    pub severity: String,
    pub message: String,
    pub detail: Option<String>,
    pub hint: Option<String>,
    /// `DbError::where_()` — a pilha de chamadas do PL/pgSQL.
    pub context: Option<String>,
    pub sql_state: Option<String>,
    pub position: Option<u32>,
    pub timestamp_ms: u64,
}

impl QueryMessage {
    /// Linha de conclusão do app (contagem + tempo). O Postgres manda só o
    /// número no `CommandComplete`, sem a tag textual, então o texto é nosso.
    pub fn status(message: impl Into<String>) -> Self {
        Self {
            kind: QueryMessageKind::Status,
            severity: "STATUS".to_string(),
            message: message.into(),
            detail: None,
            hint: None,
            context: None,
            sql_state: None,
            position: None,
            timestamp_ms: now_ms(),
        }
    }
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
