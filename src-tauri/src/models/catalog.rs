//! Contrato do catálogo de metadados com o front (espelhado em
//! `src/api/types/catalog.types.ts`). As fatias de dado (nós, busca,
//! resolução, drift) são os tipos do próprio `catalog`; aqui ficam o que é do
//! serviço: estado, eventos e o caminho que o front manda.

use serde::{Deserialize, Serialize};

pub use crate::catalog::{
    CatalogNode, CatalogStats, DriftReport, NodeKind, Page, Resolution, SearchHit, ShapeGroup,
};
use crate::catalog::CatalogPath;

/// Nome do evento global do Tauri com o progresso dos catálogos.
pub const CATALOG_EVENT: &str = "catalog-event";

/// Nó cujos filhos o front quer: `{ kind: "schemas" }`,
/// `{ kind: "schema", schema }`, `{ kind: "partitions", schema, table }` ou,
/// na árvore agrupada por formato, `{ kind: "shape", key }`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CatalogPathInput {
    Schemas,
    Schema { schema: String },
    Partitions { schema: String, table: String },
    Shape { key: String },
}

impl CatalogPathInput {
    /// Schema cujas relações precisam estar carregadas para responder.
    pub fn schema(&self) -> Option<&str> {
        match self {
            Self::Schemas | Self::Shape { .. } => None,
            Self::Schema { schema } | Self::Partitions { schema, .. } => Some(schema),
        }
    }
}

impl From<CatalogPathInput> for CatalogPath {
    fn from(input: CatalogPathInput) -> Self {
        match input {
            CatalogPathInput::Schemas => CatalogPath::Schemas,
            CatalogPathInput::Schema { schema } => CatalogPath::Schema(schema),
            CatalogPathInput::Partitions { schema, table } => CatalogPath::Partitions { schema, table },
            CatalogPathInput::Shape { key } => CatalogPath::ShapeSchemas(key),
        }
    }
}

/// Situação do catálogo de um database.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogStatus {
    pub server_id: i64,
    pub database: String,
    /// Uma sincronização está rodando (a árvore pode mostrar "atualizando")
    pub syncing: bool,
    /// Última revalidação concluída com o banco (ms epoch)
    pub fetched_at: Option<i64>,
    /// O conteúdo veio do arquivo em disco e ainda não foi revalidado
    pub from_disk: bool,
    /// Erro da última sincronização, se houve
    pub error: Option<String>,
    pub server_version: Option<String>,
    pub stats: CatalogStats,
}

/// Evento de progresso, emitido como [`CATALOG_EVENT`]. O front invalida as
/// fatias daquele database conforme o tipo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEvent {
    pub server_id: i64,
    pub database: String,
    #[serde(flatten)]
    pub kind: CatalogEventKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum CatalogEventKind {
    /// Uma sincronização começou
    Syncing,
    /// Camada 0 aplicada: a lista de schemas está disponível (e mudou assim)
    Schemas { added: Vec<String>, removed: Vec<String> },
    /// Relações aplicadas a `schemas` schemas (sincronização ou carga prioritária)
    Relations { schemas: usize },
    /// Sincronização concluída
    Ready {
        added: Vec<String>,
        removed: Vec<String>,
        /// Schemas que já estavam carregados e mudaram no banco
        changed: Vec<String>,
        fetched_at: Option<i64>,
    },
    Error { message: String },
    Cancelled,
}

/// Resultado da busca com o database de onde veio (a palette busca em todos
/// os catálogos abertos).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogSearchHit {
    pub server_id: i64,
    pub database: String,
    #[serde(flatten)]
    pub hit: SearchHit,
}

/// Diagnóstico de um catálogo ("Copiar diagnóstico" na árvore): estado,
/// última sincronização e o resumo do drift. Só contagens e tempos — nenhum
/// nome de schema, tabela, host ou usuário.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogDiagnostics {
    pub app_version: String,
    pub status: CatalogStatus,
    pub last_sync: Option<SyncDiagnostics>,
    pub drift: DriftSummary,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncDiagnostics {
    /// Quando terminou (ms epoch)
    pub at: i64,
    /// `shapeFirst` (um representante por formato) ou `bulk`
    pub strategy: String,
    pub schemas: usize,
    /// Formatos distintos no servidor (só no shape-first)
    pub shapes: Option<usize>,
    /// Schemas cujas relações vieram do banco
    pub fetched: usize,
    /// Schemas que copiaram o formato de outro
    pub shared: usize,
    pub added: usize,
    pub removed: usize,
    pub changed: usize,
    pub layer0_ms: f64,
    pub total_ms: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriftSummary {
    /// Relações do formato dominante (`None` = sem formato repetido)
    pub dominant_tables: Option<usize>,
    pub dominant_schemas: usize,
    pub divergent_groups: usize,
    pub divergent_schemas: usize,
}
