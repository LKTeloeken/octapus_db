use tauri::State;
use crate::{
    app_state::AppState,
    models::{PostgreDatabase, PostgreSchema},
    db_connection_manager::{execute_query},
};

#[tauri::command]
pub fn get_postgre_databases(state: State<AppState>, server_id: i32) -> Result<Vec<PostgreDatabase>, String> {
    let query = "SELECT * FROM pg_database WHERE datistemplate = false".to_string();

    let result = execute_query(&state, server_id, &query, None).map_err(|e| e.to_string())?;

    let mut databases = Vec::new();

    for row in result {
        if let Some(db_name) = row.get("datname") {
            databases.push(PostgreDatabase {datname: db_name.clone()});
        }
    }

    Ok(databases)
}

#[tauri::command]
pub fn get_postgre_schemas(
    state: State<AppState>,
    server_id: i32,
    database_name: String,
) -> Result<Vec<PostgreSchema>, String> {
    let query = "SELECT schema_name FROM information_schema.schemata WHERE schema_name NOT IN ('pg_toast', 'pg_catalog', 'information_schema')".to_string();

    let result = execute_query(&state, server_id, &query, Some(database_name)).map_err(|e| e.to_string())?;

    let mut schemas = Vec::new();

    for row in result {
        if let Some(schema_name) = row.get("schema_name") {
            schemas.push(PostgreSchema { name: schema_name.clone() });
        }
    }

    Ok(schemas)
}