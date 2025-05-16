use tauri::State;
use crate::{
    app_state::AppState,
    models::postgres::{PostgreSchema, PostgreTable, PostgreColumn, PostgreTrigger, PostgreIndex, PostgrePrimaryKey, PostgreForeignKey, PostgreView},
};

#[tauri::command]
pub fn get_postgre_schemas(state: State<AppState>) -> Result<Vec<PostgreSchema>, String> {
    let conn = state.conn.lock().map_err(|e | e.to_string())?;

    let query = format!(
        r#"
        SELECT schema_name
        FROM   information_schema.schemata
        WHERE  schema_name NOT IN ('pg_catalog', 'information_schema')
        ORDER  BY schema_name;"#
    );

    let mut stmt = conn.prepare(&query).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| {
        Ok(PostgreSchema {name: row.get(0)?})
    }).map_err(|e| e.to_string())?;

    let mut schemas = Vec::new();

    for row in rows {
        match row {
            Ok(schema) => schemas.push(schema),
            Err(e) => return Err(e.to_string()),
        }
    }

    Ok(schemas)
}

#[tauri::command]
pub fn get_postgre_tables(state: State<AppState>, schema: String) -> Result<Vec<PostgreTable>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;

    let query = format!(
        r#"
        SELECT table_name, table_schema, table_type
        FROM   information_schema.tables
        WHERE  table_schema = '{}'
        ORDER  BY table_name;"#,
        schema
    );

    let mut stmt = conn.prepare(&query).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| {
        Ok(PostgreTable {
            name: row.get(0)?,
            schema: row.get(1)?,
            table_type: row.get(2)?,
        })
    }).map_err(|e| e.to_string())?;

    let mut tables = Vec::new();

    for row in rows {
        match row {
            Ok(table) => tables.push(table),
            Err(e) => return Err(e.to_string()),
        }
    }

    Ok(tables)
}

#[tauri::command]
pub fn get_postgre_columns(state: State<AppState>, schema: String, table: String) -> Result<Vec<PostgreColumn>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;

    let query = format!(
        r#"
        SELECT column_name, ordinal_position, data_type, is_nullable, column_default
        FROM   information_schema.columns
        WHERE  table_schema = '{}' AND table_name = '{}'
        ORDER  BY ordinal_position;"#,
        schema, table
    );

    let mut stmt = conn.prepare(&query).map_err(|e| e.to_string())?;
    let rows = stmt.query_map([], |row| {
        Ok(PostgreColumn {
            name: row.get(0)?,
            ordinal_position: row.get(1)?,
            data_type: row.get(2)?,
            is_nullable: row.get(3)?,
            column_default: row.get(4)?,
        })
    }).map_err(|e| e.to_string())?;

    let mut columns = Vec::new();

    for row in rows {
        match row {
            Ok(column) => columns.push(column),
            Err(e) => return Err(e.to_string()),
        }
    }

    Ok(columns)
}