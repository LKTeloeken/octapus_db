// filepath: /Users/lucasteloeken/Documents/projects/octapus_db/src-tauri/src/commands/postgre_commands.rs
use tauri::State;
use crate::{
    app_state::AppState,
    models::{PostgreSchema, PostgreTable, PostgreColumn},
};
use rusqlite::Connection;
use postgres::{Client, NoTls, Error};

pub fn connect_to_postgres(server_info: &PostgreServerInfo) -> Result<Client, String> {
    let connection_string = format!(
        "host={} port={} user={} password={} dbname={}",
        server_info.host, server_info.port, server_info.username, server_info.password, server_info.database
    );

    Client::connect(&connection_string, NoTls).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_postgre_schemas(state: State<AppState>, server_info: PostgreServerInfo) -> Result<Vec<PostgreSchema>, String> {
    let mut client = connect_to_postgres(&server_info)?;

    let query = "SELECT schema_name FROM information_schema.schemata WHERE schema_name NOT IN ('pg_catalog', 'information_schema') ORDER BY schema_name;";
    let rows = client.query(query, &[]).map_err(|e| e.to_string())?;

    let schemas: Vec<PostgreSchema> = rows.iter().map(|row| {
        PostgreSchema { name: row.get(0).unwrap() }
    }).collect();

    Ok(schemas)
}

#[tauri::command]
pub fn get_postgre_tables(state: State<AppState>, server_info: PostgreServerInfo, schema: String) -> Result<Vec<PostgreTable>, String> {
    let mut client = connect_to_postgres(&server_info)?;

    let query = format!("SELECT table_name, table_schema, table_type FROM information_schema.tables WHERE table_schema = '{}' ORDER BY table_name;", schema);
    let rows = client.query(&query, &[]).map_err(|e| e.to_string())?;

    let tables: Vec<PostgreTable> = rows.iter().map(|row| {
        PostgreTable {
            name: row.get(0).unwrap(),
            schema: row.get(1).unwrap(),
            table_type: row.get(2).unwrap(),
        }
    }).collect();

    Ok(tables)
}

#[tauri::command]
pub fn get_postgre_columns(state: State<AppState>, server_info: PostgreServerInfo, schema: String, table: String) -> Result<Vec<PostgreColumn>, String> {
    let mut client = connect_to_postgres(&server_info)?;

    let query = format!(
        "SELECT column_name, ordinal_position, data_type, is_nullable, column_default FROM information_schema.columns WHERE table_schema = '{}' AND table_name = '{}' ORDER BY ordinal_position;",
        schema, table
    );
    let rows = client.query(&query, &[]).map_err(|e| e.to_string())?;

    let columns: Vec<PostgreColumn> = rows.iter().map(|row| {
        PostgreColumn {
            name: row.get(0).unwrap(),
            ordinal_position: row.get(1).unwrap(),
            data_type: row.get(2).unwrap(),
            is_nullable: row.get(3).unwrap(),
            column_default: row.get(4).unwrap(),
        }
    }).collect();

    Ok(columns)
}