use tauri::State;

use crate::models::{QueryOptions, QueryResult, StatementResult};
use crate::state::AppState;

use super::get_server;

#[tauri::command]
pub async fn execute_query(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
    query: String,
    options: Option<QueryOptions>,
) -> Result<QueryResult, String> {
    let server = get_server(state.clone(), server_id)?;

    let adapter = state
        .connections
        .get_or_connect(&server, &database)
        .map_err(|e| e.to_string())?;

    adapter
        .execute_query(&query, options.unwrap_or_default())
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn execute_statement(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
    statement: String,
) -> Result<StatementResult, String> {
    let server = get_server(state.clone(), server_id)?;

    let adapter = state
        .connections
        .get_or_connect(&server, &database)
        .map_err(|e| e.to_string())?;

    adapter
        .execute_statement(&statement)
        .await
        .map_err(|e| e.to_string())
}