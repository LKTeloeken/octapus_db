use tauri::State;

use crate::models::{ColumnInfo, DatabaseInfo, DatabaseStructure, IndexInfo, SchemaInfo, TableInfo};
use crate::state::AppState;

use super::get_server;

#[tauri::command]
pub async fn list_databases(
    state: State<'_, AppState>,
    server_id: i64,
) -> Result<Vec<DatabaseInfo>, String> {
    let server = get_server(state.clone(), server_id)?;
    let db = server.default_database.as_deref().unwrap_or("postgres");

    let adapter = state
        .connections
        .get_or_connect(&server, db)
        .map_err(|e| e.to_string())?;

    state
        .structure
        .list_databases(adapter)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_schemas(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
) -> Result<Vec<SchemaInfo>, String> {
    let server = get_server(state.clone(), server_id)?;

    let adapter = state
        .connections
        .get_or_connect(&server, &database)
        .map_err(|e| e.to_string())?;

    state
        .structure
        .list_schemas(adapter)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_tables(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
    schema: String,
) -> Result<Vec<TableInfo>, String> {
    let server = get_server(state.clone(), server_id)?;

    let adapter = state
        .connections
        .get_or_connect(&server, &database)
        .map_err(|e| e.to_string())?;

    state
        .structure
        .list_tables(adapter, &schema)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_columns(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
    schema: String,
    table: String,
) -> Result<Vec<ColumnInfo>, String> {
    let server = get_server(state.clone(), server_id)?;

    let adapter = state
        .connections
        .get_or_connect(&server, &database)
        .map_err(|e| e.to_string())?;

    state
        .structure
        .list_columns(adapter, &schema, &table)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_indexes(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
    schema: String,
    table: String,
) -> Result<Vec<IndexInfo>, String> {
    let server = get_server(state.clone(), server_id)?;

    let adapter = state
        .connections
        .get_or_connect(&server, &database)
        .map_err(|e| e.to_string())?;

    state
        .structure
        .list_indexes(adapter, &schema, &table)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_schemas_with_tables(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
) -> Result<DatabaseStructure, String> {
    let server = get_server(state.clone(), server_id)?;

    let adapter = state
        .connections
        .get_or_connect(&server, &database)
        .map_err(|e| e.to_string())?;

    adapter
        .list_schemas_with_tables()
        .await
        .map_err(|e| e.to_string())
}