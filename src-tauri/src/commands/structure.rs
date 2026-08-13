use tauri::State;

use crate::models::{ColumnInfo, DatabaseInfo, DatabaseStructure, IndexInfo, SchemaInfo, TableInfo};
use crate::state::AppState;
use crate::storage::repositories::servers;

use super::connect_adapter;

#[tauri::command]
pub async fn list_databases(
    state: State<'_, AppState>,
    server_id: i64,
) -> Result<Vec<DatabaseInfo>, String> {
    // Metadata only (no keychain) to pick the database name to connect to.
    let server = servers::get_by_id_meta(&state.storage, server_id).map_err(|e| e.to_string())?;
    let db = server
        .default_database
        .as_deref()
        .unwrap_or_else(|| server.db_type.default_database_name());

    let adapter = connect_adapter(&state, server_id, db).await?;

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
    let adapter = connect_adapter(&state, server_id, &database).await?;

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
    let adapter = connect_adapter(&state, server_id, &database).await?;

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
    let adapter = connect_adapter(&state, server_id, &database).await?;

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
    let adapter = connect_adapter(&state, server_id, &database).await?;

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
    let adapter = connect_adapter(&state, server_id, &database).await?;

    adapter
        .list_schemas_with_tables()
        .await
        .map_err(|e| e.to_string())
}