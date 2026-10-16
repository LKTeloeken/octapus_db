use tauri::State;

use crate::catalog::{NameScope, FLAT_SCHEMA};
use crate::error::Result;

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
) -> Result<Vec<DatabaseInfo>> {
    // Metadata only (no keychain) to pick the database name to connect to.
    let server = servers::get_by_id_meta(&state.storage, server_id)?;
    let db = server
        .default_database
        .as_deref()
        .unwrap_or_else(|| server.db_type.default_database_name());

    let adapter = connect_adapter(&state, server_id, db).await?;

    let databases = state.structure.list_databases(adapter).await?;

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
) -> Result<Vec<SchemaInfo>> {
    let adapter = connect_adapter(&state, server_id, &database).await?;
    let server = servers::get_by_id_meta(&state.storage, server_id)?;

    let schemas = state.structure.list_schemas(adapter).await?;

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
) -> Result<Vec<TableInfo>> {
    let adapter = connect_adapter(&state, server_id, &database).await?;

    state.structure.list_tables(adapter, &schema).await
}

#[tauri::command]
pub async fn list_columns(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
    schema: String,
    table: String,
) -> Result<Vec<ColumnInfo>> {
    let adapter = connect_adapter(&state, server_id, &database).await?;

    state.structure.list_columns(adapter, &schema, &table).await
}

#[tauri::command]
pub async fn list_indexes(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
    schema: String,
    table: String,
) -> Result<Vec<IndexInfo>> {
    let adapter = connect_adapter(&state, server_id, &database).await?;

    state.structure.list_indexes(adapter, &schema, &table).await
}

#[tauri::command]
pub async fn list_schemas_with_tables(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
) -> Result<DatabaseStructure> {
    let adapter = connect_adapter(&state, server_id, &database).await?;

    adapter.list_schemas_with_tables().await
}
/// O SQL de uma mudança de estrutura (tabela ou coluna), sem aplicar: o
/// diálogo da árvore mostra antes de confirmar.
#[tauri::command]
pub async fn preview_structure_change(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
    change: StructureChange,
) -> Result<Vec<String>> {
    let adapter = connect_adapter(&state, server_id, &database).await?;

    adapter.structure_change_sql(change).await
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
) -> Result<StatementResult> {
    let adapter = connect_adapter(&state, server_id, &database).await?;
    let schema = change.schema().unwrap_or(FLAT_SCHEMA).to_string();

    let result = adapter.apply_structure_change(change).await?;

    // A mudança já foi aplicada: falhar em recarregar o catálogo não a desfaz
    // (o refresh manual da árvore resolve)
    if let Some(entry) = state.catalog.get(server_id, &database) {
        let _ = state.catalog.refresh_schema(&entry, &schema).await;
    }
    Ok(result)
}
