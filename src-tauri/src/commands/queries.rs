use tauri::State;

use crate::models::{EditableInfo, QueryOptions, QueryResult, RowEdit, RowInsert, StatementResult};
use crate::state::AppState;

use super::connect_adapter;

#[tauri::command]
pub async fn execute_query(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
    query: String,
    options: Option<QueryOptions>,
) -> Result<QueryResult, String> {
    let adapter = connect_adapter(&state, server_id, &database).await?;

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
    let adapter = connect_adapter(&state, server_id, &database).await?;

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
    let adapter = connect_adapter(&state, server_id, &database).await?;

    adapter
        .apply_row_edits(&editable, edits)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn insert_rows(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
    editable: EditableInfo,
    rows: Vec<RowInsert>,
) -> Result<StatementResult, String> {
    let adapter = connect_adapter(&state, server_id, &database).await?;

    adapter
        .insert_rows(&editable, rows)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_rows(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
    editable: EditableInfo,
    pk_values: Vec<Vec<Option<String>>>,
) -> Result<StatementResult, String> {
    let adapter = connect_adapter(&state, server_id, &database).await?;

    adapter
        .delete_rows(&editable, pk_values)
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
    let adapter = connect_adapter(&state, server_id, &database).await?;

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
    // Cancellation only makes sense against the connection running the query.
    // If it isn't open, there is nothing to cancel — don't create a pool.
    let Some(adapter) = state.connections.get_cached(server_id, &database) else {
        return Ok(());
    };

    adapter
        .cancel_query(&query_id)
        .await
        .map_err(|e| e.to_string())
}
