use tauri::State;

use crate::models::{EditableInfo, QueryOptions, QueryResult, RowEdit, StatementResult};
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

    state
        .queries
        .execute_query(adapter, &query, options.unwrap_or_default())
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

    state
        .queries
        .execute_statement(adapter, &statement)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn apply_row_edits(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
    editable: EditableInfo,
    edits: Vec<RowEdit>,
) -> Result<StatementResult, String> {
    let server = get_server(state.clone(), server_id)?;

    let adapter = state
        .connections
        .get_or_connect(&server, &database)
        .map_err(|e| e.to_string())?;

    adapter
        .apply_row_edits(&editable, edits)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn insert_table_rows(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
    editable: EditableInfo,
    column_names: Vec<String>,
    rows: Vec<Vec<Option<String>>>,
) -> Result<StatementResult, String> {
    let server = get_server(state.clone(), server_id)?;

    let adapter = state
        .connections
        .get_or_connect(&server, &database)
        .map_err(|e| e.to_string())?;

    adapter
        .insert_table_rows(&editable, column_names, rows)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_table_rows(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
    editable: EditableInfo,
    pk_values_list: Vec<Vec<Option<String>>>,
) -> Result<StatementResult, String> {
    let server = get_server(state.clone(), server_id)?;

    let adapter = state
        .connections
        .get_or_connect(&server, &database)
        .map_err(|e| e.to_string())?;

    adapter
        .delete_table_rows(&editable, pk_values_list)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn execute_transaction(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
    statements: Vec<String>,
) -> Result<Vec<StatementResult>, String> {
    let server = get_server(state.clone(), server_id)?;

    let adapter = state
        .connections
        .get_or_connect(&server, &database)
        .map_err(|e| e.to_string())?;

    state
        .queries
        .execute_transaction(adapter, statements)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn cancel_query(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
    query_id: String,
) -> Result<(), String> {
    let server = get_server(state.clone(), server_id)?;

    let adapter = state
        .connections
        .get_or_connect(&server, &database)
        .map_err(|e| e.to_string())?;

    adapter
        .cancel_query(&query_id)
        .await
        .map_err(|e| e.to_string())
}
