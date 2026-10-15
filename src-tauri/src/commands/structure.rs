use tauri::State;

use crate::catalog::{NameScope, FLAT_SCHEMA};

use crate::models::{
    ColumnInfo, DatabaseInfo, DatabaseStructure, IndexInfo, SchemaInfo, StatementResult,
    StructureChange, TableInfo,
};
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

    let databases = state
        .structure
        .list_databases(adapter)
        .await
        .map_err(|e| e.to_string())?;

    // Escopo salvo: um database por tenant, e o usuário só quer alguns
    let scope = NameScope::parse(server.scope_databases.as_deref());
    Ok(databases
        .into_iter()
        .filter(|database| scope.allows(&database.name))
        .collect())
}

#[tauri::command]
pub async fn list_schemas(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
) -> Result<Vec<SchemaInfo>, String> {
    let adapter = connect_adapter(&state, server_id, &database).await?;
    let server = servers::get_by_id_meta(&state.storage, server_id).map_err(|e| e.to_string())?;

    let schemas = state
        .structure
        .list_schemas(adapter)
        .await
        .map_err(|e| e.to_string())?;

    let scope = NameScope::parse(server.scope_schemas.as_deref());
    Ok(schemas
        .into_iter()
        .filter(|schema| scope.allows(&schema.name))
        .collect())
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
/// O SQL de uma mudança de estrutura (tabela ou coluna), sem aplicar: o
/// diálogo da árvore mostra antes de confirmar.
#[tauri::command]
pub async fn preview_structure_change(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
    change: StructureChange,
) -> Result<Vec<String>, String> {
    let adapter = connect_adapter(&state, server_id, &database).await?;

    adapter
        .structure_change_sql(change)
        .await
        .map_err(|e| e.to_string())
}

/// Cria, renomeia ou remove uma tabela ou coluna, numa transação. O catálogo
/// aberto do database recarrega o schema afetado (o evento `catalog-event`
/// atualiza a árvore).
#[tauri::command]
pub async fn apply_structure_change(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
    change: StructureChange,
) -> Result<StatementResult, String> {
    let adapter = connect_adapter(&state, server_id, &database).await?;
    let schema = change.schema().unwrap_or(FLAT_SCHEMA).to_string();

    let result = adapter
        .apply_structure_change(change)
        .await
        .map_err(|e| e.to_string())?;

    // A mudança já foi aplicada: falhar em recarregar o catálogo não a desfaz
    // (o refresh manual da árvore resolve)
    if let Some(entry) = state.catalog.get(server_id, &database) {
        let _ = state.catalog.refresh_schema(&entry, &schema).await;
    }
    Ok(result)
}
