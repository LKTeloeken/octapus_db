//! Comandos do catálogo de metadados (refactor do catálogo, Fase 3). Respondem
//! da memória do [`CatalogService`](crate::services::CatalogService); só
//! carregar um schema que ainda não chegou ou pedir um tamanho vai ao banco, e
//! pela faixa de prioridade (nunca espera a sincronização em segundo plano).

use std::sync::Arc;

use tauri::State;

use crate::adapters::{create_catalog_source, has_catalog};
use crate::catalog::CatalogPath;
use crate::error::Error;
use crate::models::{
    CatalogDiagnostics, CatalogNode, CatalogPathInput, CatalogSearchHit, CatalogStatus, DriftReport,
    NodeKind, Page, Resolution, ShapeGroup,
};
use crate::services::{connection_identity, CatalogEntry, SourceOpener};
use crate::state::AppState;
use crate::storage::repositories::servers;

/// Teto de itens por chamada — a árvore pede janelas, não listas inteiras.
const MAX_PAGE: usize = 1_000;

/// Catálogo do database, aberto na primeira vez (do disco, se houver) com uma
/// revalidação em segundo plano. A senha só é decifrada aqui, quando a
/// entrada ainda não existe.
async fn entry_for(state: &State<'_, AppState>, server_id: i64, database: &str) -> Result<Arc<CatalogEntry>, String> {
    if let Some(entry) = state.catalog.get(server_id, database) {
        return Ok(entry);
    }

    let meta = servers::get_by_id_meta(&state.storage, server_id).map_err(|e| e.to_string())?;
    if !has_catalog(meta.db_type) {
        return Err(Error::UnsupportedDatabase(format!(
            "metadata catalog is not available for {:?}",
            meta.db_type
        ))
        .to_string());
    }

    let server = servers::get_by_id(&state.storage, server_id).map_err(|e| e.to_string())?;
    let target = database.to_string();
    let opener: SourceOpener = Arc::new(move || {
        let (server, database) = (server.clone(), target.clone());
        Box::pin(async move { create_catalog_source(&server, &database).await })
    });

    let entry = state
        .catalog
        .open(server_id, database, connection_identity(&meta), opener)
        .await;
    state.catalog.ensure_fresh(&entry, false);
    Ok(entry)
}

/// Abre o catálogo (ou revalida, se a última sincronização for antiga) e
/// devolve o estado na hora — o progresso chega pelo evento `catalog-event`.
#[tauri::command]
pub async fn catalog_open(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
) -> Result<CatalogStatus, String> {
    let entry = entry_for(&state, server_id, &database).await?;
    state.catalog.ensure_fresh(&entry, false);
    Ok(state.catalog.status(&entry))
}

/// Estado sem abrir nada (`null` se o catálogo não está em memória).
#[tauri::command]
pub fn catalog_status(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
) -> Result<Option<CatalogStatus>, String> {
    Ok(state
        .catalog
        .get(server_id, &database)
        .map(|entry| state.catalog.status(&entry)))
}

/// Filhos de um nó, filtrados e paginados. Abrir um schema que a
/// sincronização ainda não alcançou carrega só ele, na hora.
#[tauri::command]
pub async fn catalog_children(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
    path: CatalogPathInput,
    filter: Option<String>,
    offset: usize,
    limit: usize,
) -> Result<Page<CatalogNode>, String> {
    let entry = entry_for(&state, server_id, &database).await?;
    if let Some(schema) = path.schema() {
        state
            .catalog
            .ensure_schemas(&entry, &[schema.to_string()])
            .await
            .map_err(|e| e.to_string())?;
    }

    let path = CatalogPath::from(path);
    let page = entry
        .read()
        .children(&path, filter.as_deref(), offset, limit.min(MAX_PAGE));
    page.ok_or_else(|| Error::NotFound(format!("{path:?} in {database}")).to_string())
}

/// Busca fuzzy num database ou, sem `serverId`/`database`, em todos os
/// catálogos abertos (a palette). Com `schema`, só as relações desse schema
/// (nome exato — o schema fixado na palette).
#[tauri::command]
pub async fn catalog_search(
    state: State<'_, AppState>,
    query: String,
    limit: usize,
    server_id: Option<i64>,
    database: Option<String>,
    schema: Option<String>,
) -> Result<Vec<CatalogSearchHit>, String> {
    let limit = limit.min(MAX_PAGE);
    match (server_id, database) {
        (Some(server_id), Some(database)) => {
            let entry = entry_for(&state, server_id, &database).await?;
            let catalog = entry.read();
            let hits = match &schema {
                Some(schema) => catalog.search_in_schema(schema, &query, limit),
                None => catalog.search(&query, limit),
            };
            Ok(hits
                .into_iter()
                .map(|hit| CatalogSearchHit {
                    server_id,
                    database: database.clone(),
                    hit,
                })
                .collect())
        }
        _ => Ok(match &schema {
            Some(schema) => state.catalog.search_all_in_schema(schema, &query, limit),
            None => state.catalog.search_all(&query, limit),
        }),
    }
}

/// A que relação um nome sem schema se refere, seguindo o `search_path`.
#[tauri::command]
pub async fn catalog_resolve(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
    table: String,
    search_path: Vec<String>,
) -> Result<Resolution, String> {
    let entry = entry_for(&state, server_id, &database).await?;
    let path: Vec<&str> = search_path.iter().map(String::as_str).collect();
    let resolution = entry.read().resolve(&table, &path);
    Ok(resolution)
}

/// Completar por prefixo: relações de um schema (carregando-o se preciso) ou,
/// sem schema, os schemas.
#[tauri::command]
pub async fn catalog_complete(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
    schema: Option<String>,
    prefix: String,
    limit: usize,
) -> Result<Vec<CatalogNode>, String> {
    let entry = entry_for(&state, server_id, &database).await?;
    let limit = limit.min(MAX_PAGE);

    match schema {
        Some(schema) => {
            state
                .catalog
                .ensure_schemas(&entry, std::slice::from_ref(&schema))
                .await
                .map_err(|e| e.to_string())?;
            Ok(entry
                .read()
                .complete_tables(&schema, &prefix, limit)
                .unwrap_or_default())
        }
        None => Ok(entry
            .read()
            .complete_schemas(&prefix, limit)
            .into_iter()
            .map(|name| CatalogNode {
                name,
                kind: NodeKind::Schema,
                child_count: None,
                state: None,
                drift: None,
            })
            .collect()),
    }
}

/// Formato dominante dos tenants e os grupos que divergem dele.
#[tauri::command]
pub async fn catalog_drift(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
) -> Result<DriftReport, String> {
    let entry = entry_for(&state, server_id, &database).await?;
    let report = entry.read().drift();
    Ok(report)
}

/// Grupos por formato para a árvore agrupada: o molde dos tenants, as
/// variações e "outros". Vazio quando não há formato repetido.
#[tauri::command]
pub async fn catalog_shapes(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
) -> Result<Vec<ShapeGroup>, String> {
    let entry = entry_for(&state, server_id, &database).await?;
    let groups = entry.read().shape_groups();
    Ok(groups)
}

/// Diagnóstico do catálogo (estado, última sincronização, drift), só com
/// contagens e tempos.
#[tauri::command]
pub async fn catalog_diagnostics(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
) -> Result<CatalogDiagnostics, String> {
    let entry = entry_for(&state, server_id, &database).await?;
    Ok(state.catalog.diagnostics(&entry))
}

/// Refresh manual: um schema (recarrega já, pela faixa de prioridade) ou o
/// database inteiro (revalida em segundo plano, mesmo que recente).
#[tauri::command]
pub async fn catalog_refresh(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
    schema: Option<String>,
) -> Result<CatalogStatus, String> {
    let entry = entry_for(&state, server_id, &database).await?;
    match schema {
        Some(schema) => state
            .catalog
            .refresh_schema(&entry, &schema)
            .await
            .map_err(|e| e.to_string())?,
        None => {
            state.catalog.ensure_fresh(&entry, true);
        }
    }
    Ok(state.catalog.status(&entry))
}

/// Interrompe a sincronização em andamento (devolve se havia uma).
#[tauri::command]
pub async fn catalog_cancel(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
) -> Result<bool, String> {
    match state.catalog.get(server_id, &database) {
        Some(entry) => state.catalog.cancel(&entry).await.map_err(|e| e.to_string()),
        None => Ok(false),
    }
}

/// Tamanho exato de uma relação (dados + índices + TOAST), sob demanda.
#[tauri::command]
pub async fn catalog_relation_size(
    state: State<'_, AppState>,
    server_id: i64,
    database: String,
    schema: String,
    table: String,
) -> Result<Option<i64>, String> {
    let entry = entry_for(&state, server_id, &database).await?;
    state
        .catalog
        .relation_size(&entry, &schema, &table)
        .await
        .map_err(|e| e.to_string())
}
