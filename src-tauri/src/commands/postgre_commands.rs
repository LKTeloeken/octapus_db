use tauri::State;
use crate::{
    app_state::AppState,
    models::{PostgreDatabase, PostgreSchema, PostgreTable, PostgreColumn, PostgreTrigger, PostgreIndex},
    db_connection_manager::{execute_query, connect},
};

#[tauri::command]
pub async fn connect_to_server(state: State<'_, AppState>, server_id: i32, database_name: Option<String>) -> Result<bool, String> {
    connect(&state, server_id, database_name).await.map_err(|e| e.to_string())?;
    Ok(true)
}

#[tauri::command]
pub async fn get_postgre_databases(state: State<'_, AppState>, server_id: i32) -> Result<Vec<PostgreDatabase>, String> {
    let query = "SELECT * FROM pg_database WHERE datistemplate = false".to_string();
    let result = execute_query(&state, server_id, &query, None).await.map_err(|e| e.to_string())?;
    let mut databases = Vec::new();
    for row in result {
        if let Some(db_name) = row.get("datname") {
            databases.push(PostgreDatabase { datname: db_name.clone() });
        }
    }
    Ok(databases)
}

#[tauri::command]
pub async fn get_postgre_schemas(
    state: State<'_, AppState>,
    server_id: i32,
    database_name: String,
) -> Result<Vec<PostgreSchema>, String> {
    let query = "SELECT schema_name FROM information_schema.schemata WHERE schema_name NOT IN ('pg_toast', 'pg_catalog', 'information_schema')".to_string();
    let result = execute_query(&state, server_id, &query, Some(database_name)).await.map_err(|e| e.to_string())?;
    let mut schemas = Vec::new();
    for row in result {
        if let Some(schema_name) = row.get("schema_name") {
            schemas.push(PostgreSchema { name: schema_name.clone() });
        }
    }
    Ok(schemas)
}

#[tauri::command]
pub async fn get_postgre_tables(
    state: State<'_, AppState>,
    server_id: i32,
    database_name: String,
    schema_name: String,
) -> Result<Vec<PostgreTable>, String> {
    let query = format!(
        "SELECT table_name, table_schema, table_type \
         FROM information_schema.tables \
         WHERE table_schema = '{}' \
         ORDER BY table_name",
        schema_name
    );
    let result = execute_query(&state, server_id, &query, Some(database_name)).await
        .map_err(|e| e.to_string())?;

    let tables = result.into_iter().filter_map(|row| {
        Some(PostgreTable {
            name: row.get("table_name")?.clone(),
            schema: row.get("table_schema")?.clone(),
            table_type: row.get("table_type")?.clone(),
        })
    }).collect();

    Ok(tables)
}

#[tauri::command]
pub async fn get_postgre_columns(
    state: State<'_, AppState>,
    server_id: i32,
    database_name: String,
    schema_name: String,
    table_name: String,
) -> Result<Vec<PostgreColumn>, String> {
    let query = format!(
        "SELECT column_name, ordinal_position, data_type, is_nullable, column_default \
         FROM information_schema.columns \
         WHERE table_schema = '{}' AND table_name = '{}' \
         ORDER BY ordinal_position",
        schema_name, table_name
    );
    let result = execute_query(&state, server_id, &query, Some(database_name)).await
        .map_err(|e| e.to_string())?;

    let columns = result.into_iter().filter_map(|row| {
        let nullable = matches!(row.get("is_nullable")?.as_str(), "YES");
        Some(PostgreColumn {
            name: row.get("column_name")?.clone(),
            ordinal_position: row.get("ordinal_position")?.parse().unwrap_or(0),
            data_type: row.get("data_type")?.clone(),
            is_nullable: nullable,
            column_default: row.get("column_default").cloned(),
        })
    }).collect();

    Ok(columns)
}

#[tauri::command]
pub async fn get_postgre_triggers(
    state: State<'_, AppState>,
    server_id: i32,
    database_name: String,
    schema_name: String,
    table_name: String,
) -> Result<Vec<PostgreTrigger>, String> {
    let query = format!(
        "SELECT trigger_name, action_timing, event_manipulation, action_statement \
         FROM information_schema.triggers \
         WHERE event_object_schema = '{}' AND event_object_table = '{}' \
         ORDER BY trigger_name",
        schema_name, table_name
    );
    let result = execute_query(&state, server_id, &query, Some(database_name)).await
        .map_err(|e| e.to_string())?;

    let triggers = result.into_iter().filter_map(|row| {
        let events_csv = row.get("event_manipulation")?;
        let events = events_csv.split(',').map(|s| s.trim().to_string()).collect();
        Some(PostgreTrigger {
            schema_name: schema_name.clone(),
            table_name: table_name.clone(),
            name: row.get("trigger_name")?.clone(),
            action_timing: row.get("action_timing")?.clone(),
            events,
            action_statement: row.get("action_statement")?.clone(),
        })
    }).collect();

    Ok(triggers)
}

#[tauri::command]
pub async fn get_postgre_indexes(
    state: State<'_, AppState>,
    server_id: i32,
    database_name: String,
    schema_name: String,
    table_name: String,
) -> Result<Vec<PostgreIndex>, String> {
    let query = format!(
        "SELECT indexname AS name, schemaname AS schema_name, tablename AS table_name, indexdef \
         FROM pg_indexes \
         WHERE schemaname = '{}' AND tablename = '{}' \
         ORDER BY indexname",
        schema_name, table_name
    );
    let result = execute_query(&state, server_id, &query, Some(database_name)).await
        .map_err(|e| e.to_string())?;

    let indexes = result.into_iter().filter_map(|row| {
        Some(PostgreIndex {
            schema_name: row.get("schema_name")?.clone(),
            table_name: row.get("table_name")?.clone(),
            name: row.get("name")?.clone(),
            index_def: row.get("indexdef")?.clone(),
        })
    }).collect();

    Ok(indexes)
}