use serde::Serialize;

/// Snapshot of one backend returned by PostgreSQL's `pg_stat_activity`.
///
/// Timestamps are strings because their display format is owned by PostgreSQL
/// (and can include a timezone/precision that the frontend should not guess).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseProcess {
    pub pid: i32,
    pub database: Option<String>,
    pub username: Option<String>,
    pub application_name: Option<String>,
    pub client_address: Option<String>,
    pub client_port: Option<i32>,
    pub state: Option<String>,
    pub query: String,
    pub query_start: Option<String>,
    pub transaction_start: Option<String>,
    pub backend_start: Option<String>,
    pub wait_event_type: Option<String>,
    pub wait_event: Option<String>,
    pub backend_type: String,
    pub duration_ms: Option<i64>,
    pub is_own_process: bool,
}
