//! Catálogo do Mongo (refactor do catálogo, Fase 5).
//!
//! Um database Mongo não tem nível de schema: as coleções vão para o
//! [`FLAT_SCHEMA`] e uma sincronização é um `listCollections` só, com
//! `nameOnly` (sem as opções de cada coleção e sem lock de coleção) e
//! `authorizedCollections` (quem não tem `listCollections` no database vê as
//! coleções em que tem permissão). O tamanho fica para a coleção aberta
//! (`$collStats`); em massa ele custava 49 s para 5.000 coleções em rede
//! remota (`perf/catalog/BASELINE.md`, H8).

use std::future::Future;
use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use futures_util::TryStreamExt;
use mongodb::bson::{doc, Document};
use mongodb::{Client, Database};
use parking_lot::RwLock;
use tokio::sync::Notify;

use crate::catalog::{
    stable_hash, sync_flat, Catalog, CatalogSource, RelKind, SourceCanceller, SyncEvent,
    SyncReport, TableRecord, FLAT_SCHEMA,
};
use crate::error::{Error, Result};
use crate::models::Server;

use super::metadata::collection_size;

/// Conexões do cliente dedicado: uma faixa faz uma operação por vez
const POOL_SIZE: u32 = 2;

pub struct MongoSource {
    client: Client,
    database: String,
    version: String,
    /// Acorda a operação em andamento para ela desistir
    cancel: Arc<Notify>,
}

impl MongoSource {
    /// Cliente próprio, separado do pool do editor (o catálogo não disputa
    /// conexão com uma consulta longa), e já valida a conexão com o `buildInfo`.
    pub async fn connect(server: &Server, database: &str) -> Result<Self> {
        let mut options = super::client_options(server).await?;
        options.app_name = Some("octapus_db catalog".to_string());
        options.max_pool_size = Some(POOL_SIZE);
        options.min_pool_size = Some(0);

        let client = Client::with_options(options).map_err(|e| Error::Connection(e.to_string()))?;
        let info = client
            .database(database)
            .run_command(doc! { "buildInfo": 1 })
            .await?;
        let version = match info.get_str("version") {
            Ok(version) => format!("MongoDB {version}"),
            Err(_) => "MongoDB".to_string(),
        };

        Ok(Self {
            client,
            database: database.to_string(),
            version,
            cancel: Arc::default(),
        })
    }

    fn db(&self) -> Database {
        self.client.database(&self.database)
    }

    /// Roda `work` até o fim ou até um cancelamento. O driver não tem cancel
    /// request: largar o future fecha o cursor (e a conexão, se preciso).
    async fn cancellable<T>(&self, work: impl Future<Output = Result<T>>) -> Result<T> {
        tokio::select! {
            result = work => result,
            _ = self.cancel.notified() => Err(Error::Cancelled),
        }
    }

    /// As coleções do database, em ordem de bytes do nome.
    async fn list(&self) -> Result<Vec<TableRecord>> {
        let mut cursor = self
            .db()
            .run_cursor_command(doc! {
                "listCollections": 1,
                "nameOnly": true,
                "authorizedCollections": true,
            })
            .await?;

        let mut tables = Vec::new();
        while let Some(spec) = cursor.try_next().await? {
            if let Some(table) = collection_record(&spec) {
                tables.push(table);
            }
        }
        tables.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(tables)
    }
}

/// `{ name, type }` do `listCollections` → relação. Views viram view; coleção
/// comum e timeseries, tabela.
fn collection_record(spec: &Document) -> Option<TableRecord> {
    let name = spec.get_str("name").ok()?;
    let kind = match spec.get_str("type") {
        Ok("view") => RelKind::View,
        _ => RelKind::Table,
    };
    Some(TableRecord {
        name: name.to_string(),
        kind,
        partition_of: None,
    })
}

/// Muda quando entra, sai ou troca de tipo alguma coleção.
fn fingerprint(tables: &[TableRecord]) -> i64 {
    let mut parts: Vec<&[u8]> = Vec::with_capacity(tables.len() * 2);
    let kinds: Vec<[u8; 1]> = tables.iter().map(|table| [table.kind as u8]).collect();
    for (table, kind) in tables.iter().zip(&kinds) {
        parts.push(table.name.as_bytes());
        parts.push(kind);
    }
    stable_hash(&parts) as i64
}

struct MongoCanceller(Arc<Notify>);

#[async_trait]
impl SourceCanceller for MongoCanceller {
    async fn cancel(&self) -> Result<()> {
        // Sem permissão guardada: só a operação em andamento desiste
        self.0.notify_waiters();
        Ok(())
    }
}

#[async_trait]
impl CatalogSource for MongoSource {
    async fn sync(
        &mut self,
        catalog: &RwLock<Catalog>,
        on_event: &mut (dyn FnMut(SyncEvent) + Send),
    ) -> Result<SyncReport> {
        let started = Instant::now();
        let tables = self.cancellable(self.list()).await?;
        let fingerprint = fingerprint(&tables);
        Ok(sync_flat(catalog, tables, fingerprint, started, on_event))
    }

    async fn load_schemas(&mut self, catalog: &RwLock<Catalog>, schemas: &[String]) -> Result<usize> {
        // Só existe o schema sem nome
        if !schemas.iter().any(|schema| schema == FLAT_SCHEMA) {
            return Ok(0);
        }
        let tables = self.cancellable(self.list()).await?;
        let fingerprint = fingerprint(&tables);
        catalog.write().set_tables(FLAT_SCHEMA, tables, Some(fingerprint));
        Ok(1)
    }

    async fn relation_size(&mut self, _schema: &str, table: &str) -> Result<Option<i64>> {
        Ok(collection_size(&self.db(), table).await)
    }

    fn canceller(&self) -> Box<dyn SourceCanceller> {
        Box::new(MongoCanceller(Arc::clone(&self.cancel)))
    }

    fn is_closed(&self) -> bool {
        // O driver reconecta sozinho
        false
    }

    fn server_version(&self) -> String {
        self.version.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_list_collections_entries() {
        let view = collection_record(&doc! { "name": "active_users", "type": "view" }).unwrap();
        assert_eq!(view.kind, RelKind::View);
        let series = collection_record(&doc! { "name": "metrics", "type": "timeseries" }).unwrap();
        assert_eq!(series.kind, RelKind::Table);
        let plain = collection_record(&doc! { "name": "orders", "type": "collection" }).unwrap();
        assert_eq!((plain.name.as_str(), plain.kind), ("orders", RelKind::Table));
        assert!(collection_record(&doc! { "type": "collection" }).is_none());
    }

    #[test]
    fn fingerprint_follows_names_and_kinds() {
        let tables = |entries: &[(&str, RelKind)]| -> Vec<TableRecord> {
            entries
                .iter()
                .map(|(name, kind)| TableRecord {
                    name: name.to_string(),
                    kind: *kind,
                    partition_of: None,
                })
                .collect()
        };
        let base = fingerprint(&tables(&[("orders", RelKind::Table), ("users", RelKind::Table)]));
        assert_eq!(base, fingerprint(&tables(&[("orders", RelKind::Table), ("users", RelKind::Table)])));
        assert_ne!(base, fingerprint(&tables(&[("orders", RelKind::Table)])));
        assert_ne!(base, fingerprint(&tables(&[("orders", RelKind::View), ("users", RelKind::Table)])));
        // A fronteira entre nomes conta
        assert_ne!(
            fingerprint(&tables(&[("ab", RelKind::Table), ("c", RelKind::Table)])),
            fingerprint(&tables(&[("a", RelKind::Table), ("bc", RelKind::Table)]))
        );
    }
}
