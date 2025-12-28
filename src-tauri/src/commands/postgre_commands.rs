// src/commands/postgre_commands.rs

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::{
    app_state::AppState,
    db_connection_manager::{
        disconnect_database, disconnect_server, execute_query, execute_query_unlimited,
        execute_statement, get_connection, get_pool_stats, test_connection, PoolStats,
        QueryOptions, QueryResult,
    },
    models::{
        PostgreColumn, PostgreDatabase, PostgreIndex, PostgreSchema, PostgreTable, PostgreTrigger,
    },
};

// ─────────────────────────────────────────────────────────────────────────────
// Query Options from Frontend
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct QueryOptionsInput {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub count_total: Option<bool>,
    pub unlimited: Option<bool>,
}

impl From<QueryOptionsInput> for QueryOptions {
    fn from(input: QueryOptionsInput) -> Self {
        QueryOptions {
            limit: input.limit,
            offset: input.offset,
            count_total: input.count_total.unwrap_or(false),
            unlimited: input.unlimited.unwrap_or(false),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper
// ─────────────────────────────────────────────────────────────────────────────

fn get_value(result: &QueryResult, row_idx: usize, col_name: &str) -> Option<String> {
    let col_idx = result.columns.iter().position(|c| c.name == col_name)?;
    result.rows.get(row_idx)?.get(col_idx)?.clone()
}

struct RowAccessor<'a> {
    result: &'a QueryResult,
    row_idx: usize,
}

impl<'a> RowAccessor<'a> {
    fn new(result: &'a QueryResult, row_idx: usize) -> Self {
        Self { result, row_idx }
    }

    fn get(&self, col_name: &str) -> Option<String> {
        get_value(self.result, self.row_idx, col_name)
    }

    fn get_i32(&self, col_name: &str) -> Option<i32> {
        self.get(col_name)?.parse().ok()
    }
}

fn escape_sql_string(s: &str) -> String {
    s.replace('\'', "''")
}

// ----------------------------------------------------------------------------
// types
// ----------------------------------------------------------------------------

#[derive(Debug, Serialize, Clone)]
pub struct DatabaseStructure {
    pub schemas: Vec<SchemaStructure>,
}

#[derive(Debug, Serialize, Clone)]
pub struct SchemaStructure {
    pub name: String,
    pub tables: Vec<TableStructure>,
}

#[derive(Debug, Serialize, Clone)]
pub struct TableStructure {
    pub name: String,
    pub table_type: String,
    pub columns: Vec<ColumnStructure>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ColumnStructure {
    pub name: String,
    pub data_type: String,
    pub is_nullable: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// Connection Commands
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn connect_to_server(
    state: State<'_, AppState>,
    server_id: i32,
    database_name: Option<String>,
) -> Result<bool, String> {
    get_connection(&state, server_id, database_name)
        .await
        .map_err(|e| e.to_string())?;
    Ok(true)
}

#[tauri::command]
pub async fn cmd_test_connection(state: State<'_, AppState>, server_id: i32) -> Result<bool, String> {
    test_connection(&state, server_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(true)
}

#[tauri::command]
pub async fn cmd_disconnect_server(server_id: i32) -> Result<(), String> {
    disconnect_server(server_id).await;
    Ok(())
}

#[tauri::command]
pub async fn cmd_disconnect_database(
    server_id: i32,
    database_name: String,
) -> Result<(), String> {
    disconnect_database(server_id, &database_name).await;
    Ok(())
}

#[tauri::command]
pub async fn cmd_get_pool_stats(
    state: State<'_, AppState>,
    server_id: i32,
    database_name: Option<String>,
) -> Result<PoolStats, String> {
    get_pool_stats(&state, server_id, database_name)
        .await
        .map_err(|e| e.to_string())
}

// ─────────────────────────────────────────────────────────────────────────────
// Query Commands
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn run_postgre_query(
    state: State<'_, AppState>,
    server_id: i32,
    database_name: String,
    query: String,
    options: Option<QueryOptionsInput>,
) -> Result<QueryResult, String> {
    let trimmed = query.trim();

    if trimmed.is_empty() {
        return Err("Query vazia. Escreva uma instrução SQL válida.".to_string());
    }

    let opts = options.map(QueryOptions::from);

    execute_query(&state, server_id, &query, Some(database_name), opts)
        .await
        .map_err(|e| format!("Erro ao executar query: {}", e))
}

/// For exports or when you explicitly need all rows
#[tauri::command]
pub async fn run_postgre_query_unlimited(
    state: State<'_, AppState>,
    server_id: i32,
    database_name: String,
    query: String,
) -> Result<QueryResult, String> {
    execute_query_unlimited(&state, server_id, &query, Some(database_name))
        .await
        .map_err(|e| format!("Erro ao executar query: {}", e))
}

#[tauri::command]
pub async fn run_postgre_statement(
    state: State<'_, AppState>,
    server_id: i32,
    database_name: String,
    statement: String,
) -> Result<u64, String> {
    if statement.trim().is_empty() {
        return Err("Statement vazio.".to_string());
    }

    execute_statement(&state, server_id, &statement, Some(database_name))
        .await
        .map_err(|e| format!("Erro ao executar statement: {}", e))
}

// ─────────────────────────────────────────────────────────────────────────────
// Metadata Commands (these query system tables, keep them fast)
// ─────────────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn get_postgre_databases(
    state: State<'_, AppState>,
    server_id: i32,
) -> Result<Vec<PostgreDatabase>, String> {
    let query = "SELECT datname FROM pg_database WHERE datistemplate = false ORDER BY datname";

    let result = execute_query(&state, server_id, query, None, None)
        .await
        .map_err(|e| e.to_string())?;

    let databases = (0..result.row_count)
        .filter_map(|i| {
            Some(PostgreDatabase {
                datname: RowAccessor::new(&result, i).get("datname")?,
            })
        })
        .collect();

    Ok(databases)
}

#[tauri::command]
pub async fn get_postgre_schemas(
    state: State<'_, AppState>,
    server_id: i32,
    database_name: String,
) -> Result<Vec<PostgreSchema>, String> {
    let query = r#"
        SELECT schema_name 
        FROM information_schema.schemata 
        WHERE schema_name NOT IN ('pg_toast', 'pg_catalog', 'information_schema')
        ORDER BY schema_name
    "#;

    let result = execute_query(&state, server_id, query, Some(database_name), None)
        .await
        .map_err(|e| e.to_string())?;

    let schemas = (0..result.row_count)
        .filter_map(|i| {
            Some(PostgreSchema {
                name: RowAccessor::new(&result, i).get("schema_name")?,
            })
        })
        .collect();

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
        r#"
        SELECT table_name, table_schema, table_type
        FROM information_schema.tables
        WHERE table_schema = '{}'
        ORDER BY table_name
        "#,
        escape_sql_string(&schema_name)
    );

    let result = execute_query(&state, server_id, &query, Some(database_name), None)
        .await
        .map_err(|e| e.to_string())?;

    let tables = (0..result.row_count)
        .filter_map(|i| {
            let row = RowAccessor::new(&result, i);
            Some(PostgreTable {
                name: row.get("table_name")?,
                schema: row.get("table_schema")?,
                table_type: row.get("table_type")?,
            })
        })
        .collect();

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
        r#"
        SELECT column_name, ordinal_position, data_type, is_nullable, column_default
        FROM information_schema.columns
        WHERE table_schema = '{}' AND table_name = '{}'
        ORDER BY ordinal_position
        "#,
        escape_sql_string(&schema_name),
        escape_sql_string(&table_name)
    );

    let result = execute_query(&state, server_id, &query, Some(database_name), None)
        .await
        .map_err(|e| e.to_string())?;

    let columns = (0..result.row_count)
        .filter_map(|i| {
            let row = RowAccessor::new(&result, i);
            Some(PostgreColumn {
                name: row.get("column_name")?,
                ordinal_position: row.get_i32("ordinal_position").unwrap_or(0),
                data_type: row.get("data_type")?,
                is_nullable: row.get("is_nullable")? == "YES",
                column_default: row.get("column_default"),
            })
        })
        .collect();

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
        r#"
        SELECT trigger_name, action_timing, event_manipulation, action_statement
        FROM information_schema.triggers
        WHERE event_object_schema = '{}' AND event_object_table = '{}'
        ORDER BY trigger_name
        "#,
        escape_sql_string(&schema_name),
        escape_sql_string(&table_name)
    );

    let result = execute_query(&state, server_id, &query, Some(database_name), None)
        .await
        .map_err(|e| e.to_string())?;

    let triggers = (0..result.row_count)
        .filter_map(|i| {
            let row = RowAccessor::new(&result, i);
            let events: Vec<String> = row
                .get("event_manipulation")?
                .split(',')
                .map(|s| s.trim().to_string())
                .collect();

            Some(PostgreTrigger {
                schema_name: schema_name.clone(),
                table_name: table_name.clone(),
                name: row.get("trigger_name")?,
                action_timing: row.get("action_timing")?,
                events,
                action_statement: row.get("action_statement")?,
            })
        })
        .collect();

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
        r#"
        SELECT indexname AS name, schemaname AS schema_name, tablename AS table_name, indexdef
        FROM pg_indexes
        WHERE schemaname = '{}' AND tablename = '{}'
        ORDER BY indexname
        "#,
        escape_sql_string(&schema_name),
        escape_sql_string(&table_name)
    );

    let result = execute_query(&state, server_id, &query, Some(database_name), None)
        .await
        .map_err(|e| e.to_string())?;

    let indexes = (0..result.row_count)
        .filter_map(|i| {
            let row = RowAccessor::new(&result, i);
            Some(PostgreIndex {
                schema_name: row.get("schema_name")?,
                table_name: row.get("table_name")?,
                name: row.get("name")?,
                index_def: row.get("indexdef")?,
            })
        })
        .collect();

    Ok(indexes)
}

#[tauri::command]
pub async fn get_database_structure(
    state: State<'_, AppState>,
    server_id: i32,
    database_name: String,
) -> Result<DatabaseStructure, String> {
    // Single query to get all tables and columns
    let query = r#"
        SELECT 
            t.table_schema,
            t.table_name,
            t.table_type,
            c.column_name,
            c.data_type,
            c.is_nullable
        FROM information_schema.tables t
        LEFT JOIN information_schema.columns c 
            ON t.table_schema = c.table_schema 
            AND t.table_name = c.table_name
        WHERE t.table_schema NOT IN ('pg_toast', 'pg_catalog', 'information_schema')
        ORDER BY t.table_schema, t.table_name, c.ordinal_position
    "#;

    let result = execute_query(&state, server_id, query, Some(database_name.clone()), None)
        .await
        .map_err(|e| e.to_string())?;

    // Group by schema -> table -> columns
    let mut schemas_map: std::collections::HashMap<String, std::collections::HashMap<String, TableStructure>> = 
        std::collections::HashMap::new();

    for i in 0..result.row_count {
        let row = RowAccessor::new(&result, i);
        
        let schema_name = match row.get("table_schema") {
            Some(s) => s,
            None => continue,
        };
        let table_name = match row.get("table_name") {
            Some(s) => s,
            None => continue,
        };
        let table_type = row.get("table_type").unwrap_or_default();

        let schema_entry = schemas_map.entry(schema_name.clone()).or_default();
        
        let table_entry = schema_entry.entry(table_name.clone()).or_insert_with(|| {
            TableStructure {
                name: table_name.clone(),
                table_type,
                columns: vec![],
            }
        });

        if let Some(col_name) = row.get("column_name") {
            table_entry.columns.push(ColumnStructure {
                name: col_name,
                data_type: row.get("data_type").unwrap_or_default(),
                is_nullable: row.get("is_nullable").map(|v| v == "YES").unwrap_or(false),
            });
        }
    }

    let schemas: Vec<SchemaStructure> = schemas_map
        .into_iter()
        .map(|(name, tables_map)| SchemaStructure {
            name,
            tables: tables_map.into_values().collect(),
        })
        .collect();

    Ok(DatabaseStructure { schemas })
}