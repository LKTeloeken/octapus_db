//! Contrato entre o catálogo e quem o alimenta: uma fonte por tipo de banco
//! (o introspector do Postgres, e as fontes do Mongo e do SQLite). O serviço
//! só conhece este contrato — não sabe de SQL nem de rede.

use std::time::Instant;

use async_trait::async_trait;
use parking_lot::RwLock;

use super::base::Catalog;
use super::model::{LoadState, SchemaDiff, SchemaHeader, TableRecord};
use crate::error::Result;

/// Bancos sem nível de schema (Mongo, SQLite) guardam as relações num schema
/// só, sem nome. O front pede `{ kind: "schema", schema: "" }` e trata o `""`
/// como "sem schema" nas abas e na palette.
pub const FLAT_SCHEMA: &str = "";

/// Progresso de uma sincronização, para a árvore se atualizar por camada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncEvent {
    /// Camada 0 aplicada: os schemas já podem ser listados
    Schemas(SchemaDiff),
    /// Relações aplicadas (`schemas` = quantos schemas ganharam relações)
    Relations { schemas: usize },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strategy {
    /// Um representante por formato
    ShapeFirst,
    /// Todas as relações de uma vez (sem formatos no servidor, ou formatos demais)
    Bulk,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncReport {
    pub strategy: Strategy,
    pub schemas: usize,
    /// Formatos distintos no servidor (só no shape-first)
    pub shapes: Option<usize>,
    /// Schemas cujas relações vieram do banco
    pub fetched: usize,
    /// Schemas que copiaram o formato de outro
    pub shared: usize,
    pub diff: SchemaDiff,
    /// Até a camada 0 ficar disponível
    pub layer0: std::time::Duration,
    pub total: std::time::Duration,
}

/// Interrompe a operação em andamento de uma fonte, de outra task.
#[async_trait]
pub trait SourceCanceller: Send + Sync {
    async fn cancel(&self) -> Result<()>;
}

/// Uma conexão dedicada que sabe ler a estrutura de um database. Os métodos
/// pegam `&mut self`: uma operação por vez por conexão.
#[async_trait]
pub trait CatalogSource: Send {
    /// Primeira carga ou revalidação (só o que mudou), avisando por camada.
    async fn sync(
        &mut self,
        catalog: &RwLock<Catalog>,
        on_event: &mut (dyn FnMut(SyncEvent) + Send),
    ) -> Result<SyncReport>;

    /// Carrega já as relações destes schemas (pedido com prioridade).
    async fn load_schemas(&mut self, catalog: &RwLock<Catalog>, schemas: &[String]) -> Result<usize>;

    /// Tamanho exato de uma relação, sob demanda.
    async fn relation_size(&mut self, schema: &str, table: &str) -> Result<Option<i64>>;

    fn canceller(&self) -> Box<dyn SourceCanceller>;

    /// A conexão caiu: o serviço descarta a fonte e abre outra.
    fn is_closed(&self) -> bool;

    /// Versão do servidor, para o diagnóstico.
    fn server_version(&self) -> String;
}

/// Sincronização de um banco sem schemas, que lista tudo numa ida só: aplica a
/// lista no [`FLAT_SCHEMA`] e avisa as duas camadas de uma vez. Com o mesmo
/// fingerprint, as relações já carregadas ficam como estão.
pub fn sync_flat(
    catalog: &RwLock<Catalog>,
    tables: Vec<TableRecord>,
    fingerprint: i64,
    started: Instant,
    on_event: &mut (dyn FnMut(SyncEvent) + Send),
) -> SyncReport {
    let diff = catalog.write().set_schemas([SchemaHeader {
        name: FLAT_SCHEMA.to_string(),
        fingerprint: Some(fingerprint),
    }]);
    let layer0 = started.elapsed();
    on_event(SyncEvent::Schemas(diff.clone()));

    let fetched = {
        let mut guard = catalog.write();
        let fresh = guard.schema_state(FLAT_SCHEMA) == Some(LoadState::Loaded);
        if !fresh {
            guard.set_tables(FLAT_SCHEMA, tables, Some(fingerprint));
        }
        guard.set_fetched_at(chrono::Utc::now().timestamp_millis());
        usize::from(!fresh)
    };
    on_event(SyncEvent::Relations { schemas: fetched });

    SyncReport {
        strategy: Strategy::Bulk,
        schemas: 1,
        shapes: None,
        fetched,
        shared: 0,
        diff,
        layer0,
        total: started.elapsed(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::RelKind;

    fn sync(catalog: &RwLock<Catalog>, names: &[&str], fingerprint: i64) -> (SyncReport, Vec<SyncEvent>) {
        let tables = names.iter().map(|name| TableRecord::new(*name, RelKind::Table)).collect();
        let mut events = Vec::new();
        let report = sync_flat(catalog, tables, fingerprint, Instant::now(), &mut |event| events.push(event));
        (report, events)
    }

    #[test]
    fn flat_sync_loads_once_and_reloads_on_a_new_fingerprint() {
        let catalog = RwLock::new(Catalog::new());

        let (report, events) = sync(&catalog, &["orders", "users"], 1);
        assert_eq!(report.fetched, 1);
        assert_eq!(report.diff.added, vec![FLAT_SCHEMA.to_string()]);
        assert!(matches!(events[0], SyncEvent::Schemas(_)));
        assert_eq!(events[1], SyncEvent::Relations { schemas: 1 });
        assert!(catalog.read().knows_schemas());
        assert!(catalog.read().fetched_at().is_some());

        // Mesmo fingerprint: nada a recarregar
        let (report, _) = sync(&catalog, &["orders", "users"], 1);
        assert_eq!(report.fetched, 0);
        assert!(report.diff.added.is_empty() && report.diff.changed.is_empty());

        // Mudou: o schema vira `changed` e as relações novas entram
        let (report, _) = sync(&catalog, &["orders", "users", "invoices"], 2);
        assert_eq!(report.fetched, 1);
        assert_eq!(report.diff.changed, vec![FLAT_SCHEMA.to_string()]);
        assert_eq!(catalog.read().schema_state(FLAT_SCHEMA), Some(LoadState::Loaded));
        assert_eq!(catalog.read().stats().relations, 3);
    }
}
