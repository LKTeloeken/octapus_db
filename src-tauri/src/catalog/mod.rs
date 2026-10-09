//! Catálogo de metadados em memória (refactor do catálogo, Fase 1).
//!
//! O backend vira dono da estrutura dos bancos: o introspector alimenta este
//! catálogo em camadas (schemas, depois relações — de preferência um formato
//! por grupo de tenants iguais) e o front só pede fatias pequenas: filhos de um
//! nó, busca, resolução de nome e drift entre tenants. Persistido cifrado em
//! disco ([`CatalogStore`]) para abrir instantâneo no próximo uso.
//!
//! Este módulo não conhece banco, rede nem Tauri — é testado sozinho
//! (`catalog::proptests` e `perf::catalog_core`).

mod base;
mod drift;
mod interner;
mod model;
mod persist;
mod query;
mod scope;
mod search;
mod shape;
mod source;

#[cfg(test)]
mod proptests;

pub use base::Catalog;
pub use model::{
    CatalogNode, CatalogPath, CatalogStats, DriftReport, LoadState, NodeKind, Page, RelKind,
    Resolution, SchemaDiff, SchemaHeader, SearchHit, ShapeGroup, TableRecord,
};
#[cfg(test)]
pub use model::SchemaDrift;
pub use persist::{CatalogKey, CatalogStore, Loaded};
pub use scope::NameScope;
pub use source::{
    sync_flat, CatalogSource, SourceCanceller, Strategy, SyncEvent, SyncReport, FLAT_SCHEMA,
};

/// Hash estável entre execuções e versões do Rust (FNV-1a de 64 bits) — para
/// identidades e nomes de arquivo; não é criptográfico.
pub fn stable_hash(parts: &[&[u8]]) -> u64 {
    let mut hasher = shape::Fnv::new();
    for part in parts {
        hasher.write(part);
        hasher.write(&[0xff]);
    }
    hasher.finish()
}
