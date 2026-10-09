//! Serviço do catálogo de metadados (refactor do catálogo, Fase 3).
//!
//! Guarda um [`Catalog`] por (servidor, database) e é o único caminho do front
//! até ele: as consultas (filhos, busca, resolução, drift) respondem da memória,
//! sem tocar no banco. Por trás, cada catálogo tem duas faixas, cada uma com a
//! sua conexão dedicada (uma operação por vez em cada):
//!
//! - **sincronização** — primeira carga e revalidação, em segundo plano;
//! - **prioridade** — o que o usuário está esperando agora (abrir um schema que
//!   ainda não carregou, o tamanho de uma tabela). Nunca espera a sincronização,
//!   e pedidos simultâneos viram um lote só.
//!
//! Revalida **por evento** (abrir o database, refresh), nunca por polling: no
//! tier L cada revalidação custa ~0,5 s de CPU no servidor do cliente. O
//! catálogo é salvo cifrado em disco e volta de lá na próxima abertura.

use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use futures_util::future::BoxFuture;
use parking_lot::{Mutex, RwLock, RwLockReadGuard};

use crate::catalog::{
    stable_hash, Catalog, CatalogKey, CatalogSource, CatalogStore, LoadState, Loaded,
    SearchHit, SourceCanceller, Strategy, SyncEvent, SyncReport,
};
use crate::error::{Error, Result};
use crate::models::{
    CatalogDiagnostics, CatalogEvent, CatalogEventKind, CatalogSearchHit, CatalogStatus,
    ConnectionId, DriftSummary, Server, SyncDiagnostics,
};

/// Abre uma fonte (conexão dedicada) para o database de um catálogo.
pub type SourceOpener =
    Arc<dyn Fn() -> BoxFuture<'static, Result<Box<dyn CatalogSource>>> + Send + Sync>;

/// Para onde vão os eventos (na app, o `emit` do Tauri).
pub type EventSink = Arc<dyn Fn(CatalogEvent) + Send + Sync>;

#[derive(Debug, Clone)]
pub struct CatalogConfig {
    /// Abrir de novo um database sincronizado há menos que isto não revalida
    pub revalidate_after: Duration,
    /// Conexões dedicadas sem uso há mais que isto são fechadas
    pub idle_lanes_after: Duration,
    /// Teto de memória somando todos os catálogos; acima dele os menos usados
    /// saem da memória (continuam no disco)
    pub memory_budget: usize,
    /// Arquivos de catálogo sem uso há mais que isto são apagados
    pub keep_on_disk: Duration,
}

impl Default for CatalogConfig {
    fn default() -> Self {
        Self {
            revalidate_after: Duration::from_secs(60),
            idle_lanes_after: Duration::from_secs(5 * 60),
            memory_budget: 256 * 1024 * 1024,
            keep_on_disk: Duration::from_secs(30 * 24 * 3600),
        }
    }
}

/// O que identifica a conexão para o arquivo em disco: muda quando muda o que
/// se enxerga do banco (tipo, host, porta, usuário, TLS, URI, escopo de
/// schemas). Vai como hash para a URI — que pode ter senha — não ficar em
/// lugar nenhum.
pub fn connection_identity(server: &Server) -> String {
    format!(
        "{:016x}",
        stable_hash(&[
            format!("{:?}", server.db_type).as_bytes(),
            server.host.as_bytes(),
            server.port.to_string().as_bytes(),
            server.username.as_bytes(),
            &[u8::from(server.ssl_enabled)],
            server.connection_uri.as_deref().unwrap_or_default().as_bytes(),
            server.scope_schemas.as_deref().unwrap_or_default().as_bytes(),
        ])
    )
}

#[derive(Debug, Default)]
struct EntryState {
    syncing: bool,
    last_sync: Option<Instant>,
    error: Option<String>,
    /// Conteúdo veio do disco e ainda não foi revalidado
    from_disk: bool,
    server_version: Option<String>,
    /// Mudou desde o último salvamento (carga prioritária)
    dirty: bool,
    /// A última sincronização concluída e quando (ms epoch), para o diagnóstico
    last_report: Option<(SyncReport, i64)>,
}

type Lane = tokio::sync::Mutex<Option<Box<dyn CatalogSource>>>;

/// O catálogo de um database e o que o mantém vivo.
pub struct CatalogEntry {
    key: ConnectionId,
    identity: String,
    catalog: RwLock<Catalog>,
    opener: SourceOpener,
    sync_lane: Lane,
    priority_lane: Lane,
    /// Schemas pedidos com prioridade e ainda não carregados (o próximo lote)
    pending: Mutex<BTreeSet<String>>,
    state: Mutex<EntryState>,
    canceller: Mutex<Option<Arc<dyn SourceCanceller>>>,
    last_used: Mutex<Instant>,
}

impl CatalogEntry {
    /// Leitura do catálogo (conta como uso, para a manutenção).
    pub fn read(&self) -> RwLockReadGuard<'_, Catalog> {
        self.touch();
        self.catalog.read()
    }

    fn touch(&self) {
        *self.last_used.lock() = Instant::now();
    }

    fn idle_for(&self) -> Duration {
        self.last_used.lock().elapsed()
    }

    fn is_syncing(&self) -> bool {
        self.state.lock().syncing
    }

    fn store_key(&self) -> CatalogKey<'_> {
        CatalogKey {
            server_id: self.key.server_id,
            database: &self.key.database,
            identity: &self.identity,
        }
    }
}

/// O que as tarefas em segundo plano compartilham com o serviço.
#[derive(Default)]
struct Shared {
    store: OnceLock<CatalogStore>,
    sink: OnceLock<EventSink>,
    config: CatalogConfig,
}

impl Shared {
    fn emit(&self, key: &ConnectionId, kind: CatalogEventKind) {
        if let Some(sink) = self.sink.get() {
            sink(CatalogEvent {
                server_id: key.server_id,
                database: key.database.clone(),
                kind,
            });
        }
    }

    /// Grava o catálogo em disco fora do runtime async (cifrar o pior caso leva
    /// ~120 ms).
    async fn save(&self, entry: &Arc<CatalogEntry>) {
        let Some(store) = self.store.get().cloned() else {
            return;
        };
        let target = Arc::clone(entry);
        let saved = tokio::task::spawn_blocking(move || {
            let catalog = target.catalog.read();
            store.save(&target.store_key(), &catalog)
        })
        .await;
        if matches!(saved, Ok(Ok(()))) {
            entry.state.lock().dirty = false;
        }
    }
}

/// Garante uma fonte aberta (e viva) numa faixa.
async fn open_lane<'a>(
    entry: &CatalogEntry,
    lane: &'a mut Option<Box<dyn CatalogSource>>,
) -> Result<&'a mut Box<dyn CatalogSource>> {
    if lane.as_ref().is_some_and(|source| source.is_closed()) {
        *lane = None;
    }
    if lane.is_none() {
        let source = (entry.opener)().await?;
        entry.state.lock().server_version = Some(source.server_version());
        *lane = Some(source);
    }
    Ok(lane.as_mut().expect("faixa aberta acima"))
}

async fn run_sync(shared: Arc<Shared>, entry: Arc<CatalogEntry>) {
    let key = entry.key.clone();
    shared.emit(&key, CatalogEventKind::Syncing);

    let outcome = async {
        let mut lane = entry.sync_lane.lock().await;
        let source = open_lane(&entry, &mut lane).await?;
        *entry.canceller.lock() = Some(Arc::from(source.canceller()));

        let mut on_event = |event: SyncEvent| match event {
            SyncEvent::Schemas(diff) => shared.emit(
                &key,
                CatalogEventKind::Schemas {
                    added: diff.added,
                    removed: diff.removed,
                },
            ),
            SyncEvent::Relations { schemas } => {
                shared.emit(&key, CatalogEventKind::Relations { schemas })
            }
        };
        let result = source.sync(&entry.catalog, &mut on_event).await;

        entry.canceller.lock().take();
        if result.is_err() && source.is_closed() {
            *lane = None;
        }
        result
    }
    .await;

    match outcome {
        Ok(report) => {
            {
                let mut state = entry.state.lock();
                state.syncing = false;
                state.last_sync = Some(Instant::now());
                state.error = None;
                state.from_disk = false;
                state.last_report = Some((report.clone(), chrono::Utc::now().timestamp_millis()));
            }
            shared.save(&entry).await;
            let fetched_at = entry.catalog.read().fetched_at();
            shared.emit(
                &key,
                CatalogEventKind::Ready {
                    added: report.diff.added,
                    removed: report.diff.removed,
                    changed: report.diff.changed,
                    fetched_at,
                },
            );
        }
        Err(Error::Cancelled) => {
            entry.state.lock().syncing = false;
            shared.emit(&key, CatalogEventKind::Cancelled);
        }
        Err(error) => {
            {
                let mut state = entry.state.lock();
                state.syncing = false;
                state.error = Some(error.to_string());
            }
            shared.emit(
                &key,
                CatalogEventKind::Error {
                    message: error.to_string(),
                },
            );
        }
    }
}

pub struct CatalogService {
    entries: Mutex<HashMap<ConnectionId, Arc<CatalogEntry>>>,
    shared: Arc<Shared>,
}

impl Default for CatalogService {
    fn default() -> Self {
        Self::new(CatalogConfig::default())
    }
}

impl CatalogService {
    pub fn new(config: CatalogConfig) -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            shared: Arc::new(Shared {
                config,
                ..Default::default()
            }),
        }
    }

    /// Liga o disco e os eventos (uma vez, no setup da app).
    pub fn init(&self, store_dir: PathBuf, sink: EventSink) {
        let _ = self.shared.store.set(CatalogStore::new(store_dir));
        let _ = self.shared.sink.set(sink);
    }

    pub fn get(&self, server_id: i64, database: &str) -> Option<Arc<CatalogEntry>> {
        let entry = self
            .entries
            .lock()
            .get(&ConnectionId::new(server_id, database))
            .cloned()?;
        entry.touch();
        Some(entry)
    }

    /// Entrada do database, criada se preciso — com o conteúdo do disco, se
    /// houver (aparece na hora; a revalidação vem em seguida).
    pub async fn open(
        &self,
        server_id: i64,
        database: &str,
        identity: String,
        opener: SourceOpener,
    ) -> Arc<CatalogEntry> {
        if let Some(entry) = self.get(server_id, database) {
            return entry;
        }

        let from_disk = match self.shared.store.get().cloned() {
            Some(store) => {
                let (database, identity) = (database.to_string(), identity.clone());
                tokio::task::spawn_blocking(move || {
                    store.load(&CatalogKey {
                        server_id,
                        database: &database,
                        identity: &identity,
                    })
                })
                .await
                .ok()
                .and_then(Result::ok)
            }
            None => None,
        };
        let (catalog, from_disk) = match from_disk {
            Some(Loaded::Hit(catalog)) => (*catalog, true),
            _ => (Catalog::new(), false),
        };

        let key = ConnectionId::new(server_id, database);
        let entry = Arc::new(CatalogEntry {
            key: key.clone(),
            identity,
            catalog: RwLock::new(catalog),
            opener,
            sync_lane: tokio::sync::Mutex::new(None),
            priority_lane: tokio::sync::Mutex::new(None),
            pending: Mutex::new(BTreeSet::new()),
            state: Mutex::new(EntryState {
                from_disk,
                ..Default::default()
            }),
            canceller: Mutex::new(None),
            last_used: Mutex::new(Instant::now()),
        });

        // Duas aberturas simultâneas: fica a primeira
        Arc::clone(self.entries.lock().entry(key).or_insert(entry))
    }

    /// Dispara uma sincronização em segundo plano se nenhuma estiver rodando e
    /// a última for antiga (ou `force`). Devolve se disparou.
    pub fn ensure_fresh(&self, entry: &Arc<CatalogEntry>, force: bool) -> bool {
        {
            let mut state = entry.state.lock();
            if state.syncing {
                return false;
            }
            let recent = state
                .last_sync
                .is_some_and(|at| at.elapsed() < self.shared.config.revalidate_after);
            if recent && !force {
                return false;
            }
            state.syncing = true;
        }

        tokio::spawn(run_sync(Arc::clone(&self.shared), Arc::clone(entry)));
        true
    }

    /// Garante as relações destes schemas (o usuário está esperando). Pedidos
    /// simultâneos entram no mesmo lote; schemas já carregados (mesmo velhos)
    /// não vão ao banco.
    pub async fn ensure_schemas(&self, entry: &Arc<CatalogEntry>, schemas: &[String]) -> Result<()> {
        // Com a lista de schemas conhecida, um nome que não está nela não existe
        // (um alias digitado no editor, por exemplo): não vale ir ao banco a cada tecla
        let needs_load = |catalog: &Catalog, schema: &str| match catalog.schema_state(schema) {
            Some(LoadState::Loaded | LoadState::Stale) => false,
            Some(LoadState::Unloaded) => true,
            None => !catalog.knows_schemas(),
        };

        {
            let catalog = entry.read();
            let missing: Vec<&String> = schemas.iter().filter(|s| needs_load(&catalog, s)).collect();
            if missing.is_empty() {
                return Ok(());
            }
            entry.pending.lock().extend(missing.into_iter().cloned());
        }

        let mut lane = entry.priority_lane.lock().await;
        let batch: Vec<String> = {
            let catalog = entry.catalog.read();
            let mut pending = entry.pending.lock();
            let batch = pending.iter().filter(|s| needs_load(&catalog, s)).cloned().collect();
            pending.clear();
            batch
        };
        if batch.is_empty() {
            // Outro pedido já carregou o nosso
            return Ok(());
        }

        let source = open_lane(entry, &mut lane).await?;
        let result = source.load_schemas(&entry.catalog, &batch).await;
        if result.is_err() && source.is_closed() {
            *lane = None;
        }
        let loaded = result?;

        entry.state.lock().dirty = true;
        self.shared
            .emit(&entry.key, CatalogEventKind::Relations { schemas: loaded });
        Ok(())
    }

    /// Tamanho exato de uma relação, pela faixa de prioridade.
    pub async fn relation_size(&self, entry: &Arc<CatalogEntry>, schema: &str, table: &str) -> Result<Option<i64>> {
        entry.touch();
        let mut lane = entry.priority_lane.lock().await;
        let source = open_lane(entry, &mut lane).await?;
        let result = source.relation_size(schema, table).await;
        if result.is_err() && source.is_closed() {
            *lane = None;
        }
        result
    }

    /// Recarrega um schema (refresh manual): continua visível com o conteúdo
    /// antigo até a nova carga chegar.
    pub async fn refresh_schema(&self, entry: &Arc<CatalogEntry>, schema: &str) -> Result<()> {
        entry.catalog.write().invalidate(schema);
        let mut lane = entry.priority_lane.lock().await;
        let source = open_lane(entry, &mut lane).await?;
        let result = source.load_schemas(&entry.catalog, &[schema.to_string()]).await;
        if result.is_err() && source.is_closed() {
            *lane = None;
        }
        let loaded = result?;
        entry.state.lock().dirty = true;
        self.shared
            .emit(&entry.key, CatalogEventKind::Relations { schemas: loaded });
        Ok(())
    }

    /// Interrompe a sincronização em andamento. Devolve se havia uma.
    pub async fn cancel(&self, entry: &Arc<CatalogEntry>) -> Result<bool> {
        let canceller = entry.canceller.lock().clone();
        match canceller {
            Some(canceller) => canceller.cancel().await.map(|_| true),
            None => Ok(false),
        }
    }

    pub fn status(&self, entry: &CatalogEntry) -> CatalogStatus {
        let catalog = entry.catalog.read();
        let state = entry.state.lock();
        CatalogStatus {
            server_id: entry.key.server_id,
            database: entry.key.database.clone(),
            syncing: state.syncing,
            fetched_at: catalog.fetched_at(),
            from_disk: state.from_disk,
            error: state.error.clone(),
            server_version: state.server_version.clone(),
            stats: catalog.stats(),
        }
    }

    /// O que ajuda a entender um catálogo lento ou estranho, sem nomes de
    /// schema ou tabela (o usuário cola isto numa issue).
    pub fn diagnostics(&self, entry: &CatalogEntry) -> CatalogDiagnostics {
        let status = self.status(entry);
        let last_sync = entry.state.lock().last_report.clone().map(|(report, at)| SyncDiagnostics {
            at,
            strategy: match report.strategy {
                Strategy::ShapeFirst => "shapeFirst",
                Strategy::Bulk => "bulk",
            }
            .to_string(),
            schemas: report.schemas,
            shapes: report.shapes,
            fetched: report.fetched,
            shared: report.shared,
            added: report.diff.added.len(),
            removed: report.diff.removed.len(),
            changed: report.diff.changed.len(),
            layer0_ms: report.layer0.as_secs_f64() * 1000.0,
            total_ms: report.total.as_secs_f64() * 1000.0,
        });
        let drift = {
            let report = entry.catalog.read().drift();
            DriftSummary {
                dominant_tables: report.dominant.as_ref().map(|dominant| dominant.tables),
                dominant_schemas: report.dominant.as_ref().map_or(0, |dominant| dominant.schemas.total),
                divergent_groups: report.divergent.len(),
                divergent_schemas: report.divergent.iter().map(|group| group.schemas.total).sum(),
            }
        };
        CatalogDiagnostics {
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            status,
            last_sync,
            drift,
        }
    }

    /// Busca em todos os catálogos abertos (a palette), os melhores primeiro.
    pub fn search_all(&self, query: &str, limit: usize) -> Vec<CatalogSearchHit> {
        self.search_each(limit, |catalog| catalog.search(query, limit))
    }

    /// Como `search_all`, mas só nas relações do schema `schema` (exato) — o
    /// schema fixado na palette.
    pub fn search_all_in_schema(&self, schema: &str, query: &str, limit: usize) -> Vec<CatalogSearchHit> {
        self.search_each(limit, |catalog| catalog.search_in_schema(schema, query, limit))
    }

    fn search_each(&self, limit: usize, search: impl Fn(&Catalog) -> Vec<SearchHit>) -> Vec<CatalogSearchHit> {
        let entries: Vec<Arc<CatalogEntry>> = self.entries.lock().values().cloned().collect();
        let mut hits: Vec<CatalogSearchHit> = entries
            .iter()
            .flat_map(|entry| {
                search(&entry.read())
                    .into_iter()
                    .map(|hit| CatalogSearchHit {
                        server_id: entry.key.server_id,
                        database: entry.key.database.clone(),
                        hit,
                    })
                    .collect::<Vec<_>>()
            })
            .collect();
        hits.sort_by(|a, b| {
            b.hit
                .score
                .cmp(&a.hit.score)
                .then_with(|| a.hit.name.len().cmp(&b.hit.name.len()))
                .then_with(|| a.hit.name.cmp(&b.hit.name))
                .then_with(|| (a.server_id, &a.database).cmp(&(b.server_id, &b.database)))
        });
        hits.truncate(limit);
        hits
    }

    /// Servidor editado ou excluído: tira os catálogos da memória (fechando as
    /// conexões) e apaga os arquivos.
    pub fn forget_server(&self, server_id: i64) {
        self.entries
            .lock()
            .retain(|key, _| key.server_id != server_id);
        if let Some(store) = self.shared.store.get() {
            let _ = store.purge_server(server_id);
        }
    }

    /// Apaga do disco os catálogos sem uso há muito tempo.
    pub fn purge_expired(&self) -> Result<usize> {
        match self.shared.store.get() {
            Some(store) => store.purge_expired(self.shared.config.keep_on_disk),
            None => Ok(0),
        }
    }

    /// Rodada periódica: salva o que ficou pendente, fecha conexões ociosas e
    /// tira da memória os catálogos menos usados quando passa do teto.
    pub async fn maintenance(&self) {
        let entries: Vec<Arc<CatalogEntry>> = self.entries.lock().values().cloned().collect();
        let config = &self.shared.config;

        for entry in &entries {
            if entry.state.lock().dirty && !entry.is_syncing() {
                self.shared.save(entry).await;
            }
            if entry.idle_for() > config.idle_lanes_after {
                // try_lock: uma faixa ocupada não está ociosa
                for lane in [&entry.sync_lane, &entry.priority_lane] {
                    if let Ok(mut lane) = lane.try_lock() {
                        lane.take();
                    }
                }
            }
        }

        let mut total: usize = entries
            .iter()
            .map(|entry| entry.catalog.read().stats().approx_heap_bytes)
            .sum();
        if total <= config.memory_budget {
            return;
        }

        let mut by_age = entries;
        by_age.sort_by_key(|entry| std::cmp::Reverse(entry.idle_for()));
        for entry in by_age {
            if total <= config.memory_budget {
                break;
            }
            if entry.is_syncing() {
                continue;
            }
            if entry.state.lock().dirty {
                self.shared.save(&entry).await;
            }
            total -= entry.catalog.read().stats().approx_heap_bytes;
            self.entries.lock().remove(&entry.key);
        }
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.entries.lock().len()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use async_trait::async_trait;

    use super::*;
    use crate::catalog::{
        RelKind, SchemaDiff, SchemaHeader, Strategy, SyncReport, TableRecord,
    };
    use crate::models::{CatalogPathInput, DatabaseType};
    use crate::storage::vault;

    /// O "banco" da fonte falsa: schema → (tabelas, fingerprint).
    type FakeDb = Arc<Mutex<BTreeMap<String, (Vec<&'static str>, i64)>>>;

    #[derive(Clone, Default)]
    struct Probe {
        calls: Arc<Mutex<Vec<String>>>,
        opens: Arc<AtomicUsize>,
        fail_next_sync: Arc<AtomicBool>,
    }

    struct FakeSource {
        db: FakeDb,
        probe: Probe,
        delay: Duration,
        cancelled: Arc<AtomicBool>,
        closed: bool,
    }

    struct FakeCanceller(Arc<AtomicBool>);

    #[async_trait]
    impl SourceCanceller for FakeCanceller {
        async fn cancel(&self) -> Result<()> {
            self.0.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    impl FakeSource {
        async fn wait(&self) -> Result<()> {
            let deadline = Instant::now() + self.delay;
            while Instant::now() < deadline {
                if self.cancelled.swap(false, Ordering::SeqCst) {
                    return Err(Error::Cancelled);
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            Ok(())
        }

        fn records(tables: &[&'static str]) -> Vec<TableRecord> {
            tables.iter().map(|t| TableRecord::new(*t, RelKind::Table)).collect()
        }
    }

    #[async_trait]
    impl CatalogSource for FakeSource {
        async fn sync(
            &mut self,
            catalog: &RwLock<Catalog>,
            on_event: &mut (dyn FnMut(SyncEvent) + Send),
        ) -> Result<SyncReport> {
            self.probe.calls.lock().push("sync".into());
            if self.probe.fail_next_sync.swap(false, Ordering::SeqCst) {
                self.closed = true;
                return Err(Error::Connection("connection reset".into()));
            }
            self.wait().await?;

            let db = self.db.lock().clone();
            let layer0 = catalog.write().set_schemas(db.keys().map(|name| SchemaHeader {
                name: name.clone(),
                fingerprint: None,
            }));
            on_event(SyncEvent::Schemas(layer0.clone()));
            self.wait().await?;

            let mut catalog = catalog.write();
            let changed = catalog
                .set_schemas(db.iter().map(|(name, (_, fp))| SchemaHeader {
                    name: name.clone(),
                    fingerprint: Some(*fp),
                }))
                .changed;
            let pending = catalog.pending_schemas();
            for name in &pending {
                let (tables, fp) = &db[name];
                catalog.set_tables(name, Self::records(tables), Some(*fp));
            }
            catalog.set_fetched_at(1_700_000_000_000);
            on_event(SyncEvent::Relations { schemas: pending.len() });

            Ok(SyncReport {
                strategy: Strategy::ShapeFirst,
                schemas: db.len(),
                shapes: None,
                fetched: pending.len(),
                shared: 0,
                diff: SchemaDiff {
                    changed,
                    ..layer0
                },
                layer0: Duration::ZERO,
                total: Duration::ZERO,
            })
        }

        async fn load_schemas(&mut self, catalog: &RwLock<Catalog>, schemas: &[String]) -> Result<usize> {
            self.probe.calls.lock().push(format!("load:{}", schemas.join(",")));
            self.wait().await?;
            let db = self.db.lock().clone();
            let mut catalog = catalog.write();
            let mut loaded = 0;
            for name in schemas {
                if let Some((tables, fp)) = db.get(name) {
                    catalog.set_tables(name, Self::records(tables), Some(*fp));
                    loaded += 1;
                }
            }
            Ok(loaded)
        }

        async fn relation_size(&mut self, _schema: &str, _table: &str) -> Result<Option<i64>> {
            Ok(Some(42))
        }

        fn canceller(&self) -> Box<dyn SourceCanceller> {
            Box::new(FakeCanceller(Arc::clone(&self.cancelled)))
        }

        fn is_closed(&self) -> bool {
            self.closed
        }

        fn server_version(&self) -> String {
            "Fake 1.0".into()
        }
    }

    fn fake_db() -> FakeDb {
        let mut db = BTreeMap::new();
        for tenant in ["t1", "t2", "t3"] {
            db.insert(tenant.to_string(), (vec!["orders", "customers"], 1));
        }
        db.insert("public".to_string(), (vec!["plans"], 2));
        Arc::new(Mutex::new(db))
    }

    fn opener(db: &FakeDb, probe: &Probe, delay: Duration) -> SourceOpener {
        let (db, probe) = (Arc::clone(db), probe.clone());
        Arc::new(move || {
            probe.opens.fetch_add(1, Ordering::SeqCst);
            let source = FakeSource {
                db: Arc::clone(&db),
                probe: probe.clone(),
                delay,
                cancelled: Arc::new(AtomicBool::new(false)),
                closed: false,
            };
            Box::pin(async move { Ok(Box::new(source) as Box<dyn CatalogSource>) })
        })
    }

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "octapus-catalog-service-{label}-{}-{:?}",
                std::process::id(),
                Instant::now()
            ));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    type Events = Arc<Mutex<Vec<CatalogEvent>>>;

    fn service(dir: &TempDir, config: CatalogConfig) -> (CatalogService, Events) {
        vault::init_for_tests();
        let service = CatalogService::new(config);
        let events: Events = Arc::default();
        let sink = Arc::clone(&events);
        service.init(dir.0.clone(), Arc::new(move |event| sink.lock().push(event)));
        (service, events)
    }

    async fn wait_until(what: &str, mut condition: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !condition() {
            assert!(Instant::now() < deadline, "esperando: {what}");
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }

    fn kinds(events: &Events) -> Vec<&'static str> {
        events
            .lock()
            .iter()
            .map(|event| match event.kind {
                CatalogEventKind::Syncing => "syncing",
                CatalogEventKind::Schemas { .. } => "schemas",
                CatalogEventKind::Relations { .. } => "relations",
                CatalogEventKind::Ready { .. } => "ready",
                CatalogEventKind::Error { .. } => "error",
                CatalogEventKind::Cancelled => "cancelled",
            })
            .collect()
    }

    fn ready_count(events: &Events) -> usize {
        kinds(events).iter().filter(|kind| **kind == "ready").count()
    }

    #[tokio::test]
    async fn open_syncs_in_layers_and_persists() {
        let dir = TempDir::new("open");
        let (service, events) = service(&dir, CatalogConfig::default());
        let (db, probe) = (fake_db(), Probe::default());

        let entry = service.open(1, "app", "id".into(), opener(&db, &probe, Duration::ZERO)).await;
        assert!(service.ensure_fresh(&entry, false));
        wait_until("ready", || ready_count(&events) == 1).await;

        assert_eq!(kinds(&events), ["syncing", "schemas", "relations", "ready"]);
        let status = service.status(&entry);
        assert!(!status.syncing && !status.from_disk);
        assert_eq!(status.stats.loaded, 4);
        assert_eq!(status.server_version.as_deref(), Some("Fake 1.0"));
        assert!(CatalogStore::new(&dir.0)
            .path(&entry.store_key())
            .exists());

        // A mesma entrada para a mesma conexão
        let again = service.open(1, "app", "id".into(), opener(&db, &probe, Duration::ZERO)).await;
        assert!(Arc::ptr_eq(&entry, &again));
    }

    #[tokio::test]
    async fn reopening_comes_from_disk_then_revalidates() {
        let dir = TempDir::new("disk");
        let (db, probe) = (fake_db(), Probe::default());
        {
            let (service, events) = service(&dir, CatalogConfig::default());
            let entry = service.open(1, "app", "id".into(), opener(&db, &probe, Duration::ZERO)).await;
            service.ensure_fresh(&entry, false);
            wait_until("ready", || ready_count(&events) == 1).await;
        }

        let (service, events) = service(&dir, CatalogConfig::default());
        let entry = service.open(1, "app", "id".into(), opener(&db, &probe, Duration::ZERO)).await;
        let status = service.status(&entry);
        assert!(status.from_disk);
        assert_eq!(status.stats.loaded, 4, "árvore pronta antes de ir ao banco");

        service.ensure_fresh(&entry, false);
        wait_until("ready", || ready_count(&events) == 1).await;
        assert!(!service.status(&entry).from_disk);

        // Identidade diferente (servidor editado): o arquivo não vale
        let other = service.open(1, "other", "id".into(), opener(&db, &probe, Duration::ZERO)).await;
        assert!(!service.status(&other).from_disk);
    }

    #[tokio::test]
    async fn revalidates_by_event_and_only_once_at_a_time() {
        let dir = TempDir::new("policy");
        let (service, events) = service(&dir, CatalogConfig::default());
        let (db, probe) = (fake_db(), Probe::default());
        let entry = service
            .open(1, "app", "id".into(), opener(&db, &probe, Duration::from_millis(50)))
            .await;

        assert!(service.ensure_fresh(&entry, false));
        assert!(!service.ensure_fresh(&entry, true), "já tem uma rodando");
        wait_until("ready", || ready_count(&events) == 1).await;

        assert!(!service.ensure_fresh(&entry, false), "acabou de sincronizar");
        db.lock().get_mut("t2").unwrap().1 = 99;
        assert!(service.ensure_fresh(&entry, true));
        wait_until("ready 2", || ready_count(&events) == 2).await;

        let last = events.lock().last().cloned().unwrap();
        assert!(matches!(
            last.kind,
            CatalogEventKind::Ready { ref changed, .. } if changed == &["t2".to_string()]
        ));
        assert_eq!(probe.calls.lock().iter().filter(|c| *c == "sync").count(), 2);
        assert_eq!(probe.opens.load(Ordering::SeqCst), 1, "a conexão da faixa é reaproveitada");
    }

    #[tokio::test]
    async fn priority_requests_batch_and_skip_what_is_loaded() {
        let dir = TempDir::new("priority");
        let (service, _) = service(&dir, CatalogConfig::default());
        let (db, probe) = (fake_db(), Probe::default());
        let entry = service
            .open(1, "app", "id".into(), opener(&db, &probe, Duration::from_millis(60)))
            .await;

        let (first, second, third) = (
            vec!["t1".to_string()],
            vec!["t2".to_string()],
            vec!["t1".to_string(), "t3".to_string(), "missing".to_string()],
        );
        let (a, b, c) = tokio::join!(
            service.ensure_schemas(&entry, &first),
            service.ensure_schemas(&entry, &second),
            service.ensure_schemas(&entry, &third),
        );
        a.unwrap();
        b.unwrap();
        c.unwrap();

        let loads: Vec<String> = probe.calls.lock().clone();
        assert!(loads.len() <= 2, "{loads:?}");
        let loaded: BTreeSet<&str> = loads
            .iter()
            .flat_map(|call| call.trim_start_matches("load:").split(','))
            .collect();
        assert!(loaded.is_superset(&BTreeSet::from(["t1", "t2", "t3"])));
        assert_eq!(
            loads.iter().filter(|call| call.contains("t1")).count(),
            1,
            "t1 não vai duas vezes ao banco: {loads:?}"
        );

        let before = probe.calls.lock().len();
        service.ensure_schemas(&entry, &["t2".into()]).await.unwrap();
        assert_eq!(probe.calls.lock().len(), before, "já carregado: não vai ao banco");
        assert_eq!(service.relation_size(&entry, "t1", "orders").await.unwrap(), Some(42));
    }

    #[tokio::test]
    async fn unknown_schemas_skip_the_database_once_the_list_is_known() {
        let dir = TempDir::new("unknown");
        let (service, events) = service(&dir, CatalogConfig::default());
        let (db, probe) = (fake_db(), Probe::default());
        let entry = service.open(1, "app", "id".into(), opener(&db, &probe, Duration::ZERO)).await;

        // Antes da camada 0 não dá para saber: tenta no banco
        service.ensure_schemas(&entry, &["u".into()]).await.unwrap();
        assert_eq!(probe.calls.lock().as_slice(), ["load:u"]);

        service.ensure_fresh(&entry, false);
        wait_until("ready", || ready_count(&events) == 1).await;
        let before = probe.calls.lock().len();
        service.ensure_schemas(&entry, &["u".into()]).await.unwrap();
        assert_eq!(probe.calls.lock().len(), before, "alias não vira consulta ao banco");
    }

    #[tokio::test]
    async fn priority_does_not_wait_for_the_sync() {
        let dir = TempDir::new("lanes");
        let (service, events) = service(&dir, CatalogConfig::default());
        let (db, probe) = (fake_db(), Probe::default());
        let entry = service
            .open(1, "app", "id".into(), opener(&db, &probe, Duration::from_millis(400)))
            .await;

        // A sincronização leva ~800 ms; a carga prioritária, ~400
        service.ensure_fresh(&entry, false);
        let start = Instant::now();
        service.ensure_schemas(&entry, &["t3".into()]).await.unwrap();
        assert!(start.elapsed() < Duration::from_millis(700), "{:?}", start.elapsed());
        assert_eq!(entry.read().schema_state("t3"), Some(LoadState::Loaded));
        assert_eq!(probe.opens.load(Ordering::SeqCst), 2, "uma conexão por faixa");

        wait_until("ready", || ready_count(&events) == 1).await;
    }

    #[tokio::test]
    async fn cancel_and_errors_leave_a_usable_entry() {
        let dir = TempDir::new("errors");
        let (service, events) = service(&dir, CatalogConfig::default());
        let (db, probe) = (fake_db(), Probe::default());
        let entry = service
            .open(1, "app", "id".into(), opener(&db, &probe, Duration::from_millis(200)))
            .await;

        service.ensure_fresh(&entry, false);
        wait_until("syncing", || entry.canceller.lock().is_some()).await;
        assert!(service.cancel(&entry).await.unwrap());
        wait_until("cancelled", || kinds(&events).contains(&"cancelled")).await;
        assert!(!service.status(&entry).syncing);
        assert!(!service.cancel(&entry).await.unwrap(), "nada rodando");

        // A conexão cai no meio: erro no status e a faixa reabre na próxima
        probe.fail_next_sync.store(true, Ordering::SeqCst);
        service.ensure_fresh(&entry, true);
        wait_until("error", || kinds(&events).contains(&"error")).await;
        assert_eq!(
            service.status(&entry).error.as_deref(),
            Some("Connection error: connection reset")
        );

        service.ensure_fresh(&entry, true);
        wait_until("ready", || ready_count(&events) == 1).await;
        assert!(service.status(&entry).error.is_none());
        assert_eq!(probe.opens.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn forgetting_a_server_drops_memory_and_files() {
        let dir = TempDir::new("forget");
        let (service, events) = service(&dir, CatalogConfig::default());
        let (db, probe) = (fake_db(), Probe::default());
        for (server, database) in [(1, "a"), (1, "b"), (2, "a")] {
            let entry = service
                .open(server, database, "id".into(), opener(&db, &probe, Duration::ZERO))
                .await;
            service.ensure_fresh(&entry, false);
        }
        wait_until("ready ×3", || ready_count(&events) == 3).await;

        service.forget_server(1);
        assert_eq!(service.len(), 1);
        assert!(service.get(1, "a").is_none());
        let files = std::fs::read_dir(&dir.0).unwrap().count();
        assert_eq!(files, 1);
    }

    #[tokio::test]
    async fn maintenance_closes_idle_lanes_and_respects_the_memory_budget() {
        let dir = TempDir::new("maintenance");
        let (service, events) = service(
            &dir,
            CatalogConfig {
                idle_lanes_after: Duration::ZERO,
                memory_budget: 1,
                ..Default::default()
            },
        );
        let (db, probe) = (fake_db(), Probe::default());
        let first = service.open(1, "a", "id".into(), opener(&db, &probe, Duration::ZERO)).await;
        service.ensure_fresh(&first, false);
        wait_until("ready", || ready_count(&events) == 1).await;
        // Carga prioritária deixa a entrada "suja": a manutenção salva antes de soltar
        service.ensure_schemas(&first, &["t1".into()]).await.unwrap();

        tokio::time::sleep(Duration::from_millis(5)).await;
        service.maintenance().await;
        assert_eq!(service.len(), 0, "acima do teto: sai da memória");
        assert!(first.sync_lane.try_lock().unwrap().is_none(), "conexões ociosas fechadas");

        // Volta do disco na próxima abertura
        let again = service.open(1, "a", "id".into(), opener(&db, &probe, Duration::ZERO)).await;
        assert!(service.status(&again).from_disk);
    }

    #[tokio::test]
    async fn searches_every_open_catalog() {
        let dir = TempDir::new("search");
        let (service, events) = service(&dir, CatalogConfig::default());
        let (db, probe) = (fake_db(), Probe::default());
        for database in ["a", "b"] {
            let entry = service
                .open(1, database, "id".into(), opener(&db, &probe, Duration::ZERO))
                .await;
            service.ensure_fresh(&entry, false);
        }
        wait_until("ready ×2", || ready_count(&events) == 2).await;

        let hits = service.search_all("orders", 10);
        let orders: Vec<_> = hits.iter().filter(|hit| hit.hit.name == "orders").collect();
        assert_eq!(orders.len(), 2);
        assert_eq!((orders[0].database.as_str(), orders[1].database.as_str()), ("a", "b"));
        assert_eq!(orders[0].hit.schemas.as_ref().unwrap().total, 3);
    }

    #[tokio::test]
    async fn diagnostics_have_counts_and_timings_but_no_names() {
        let dir = TempDir::new("diag");
        let (service, events) = service(&dir, CatalogConfig::default());
        let (db, probe) = (fake_db(), Probe::default());
        let entry = service.open(1, "app", "id".into(), opener(&db, &probe, Duration::ZERO)).await;

        // Antes de sincronizar: sem última sincronização
        assert!(service.diagnostics(&entry).last_sync.is_none());

        service.ensure_fresh(&entry, false);
        wait_until("ready", || ready_count(&events) == 1).await;
        let diagnostics = service.diagnostics(&entry);
        let sync = diagnostics.last_sync.as_ref().unwrap();
        assert_eq!((sync.strategy.as_str(), sync.schemas, sync.added), ("shapeFirst", 4, 4));
        assert_eq!(diagnostics.drift.dominant_tables, Some(2));
        assert_eq!(diagnostics.drift.dominant_schemas, 3);
        assert_eq!(diagnostics.app_version, env!("CARGO_PKG_VERSION"));

        let json = serde_json::to_string(&diagnostics).unwrap();
        for name in ["orders", "customers", "t1", "public", "plans"] {
            assert!(!json.contains(&format!("\"{name}\"")), "vazou {name}: {json}");
        }
    }

    #[test]
    fn identity_follows_what_changes_the_view() {
        let server = Server {
            id: Some(1),
            name: "n".into(),
            db_type: DatabaseType::Postgres,
            host: "db".into(),
            port: 5432,
            username: "app".into(),
            password: "secret".into(),
            default_database: None,
            ssl_enabled: false,
            connection_uri: None,
            scope_databases: None,
            scope_schemas: None,
            created_at: 0,
        };
        let base = connection_identity(&server);
        assert_eq!(base, connection_identity(&Server { name: "outro".into(), password: "x".into(), ..server.clone() }));
        for changed in [
            Server { host: "db2".into(), ..server.clone() },
            Server { port: 5433, ..server.clone() },
            Server { username: "ro".into(), ..server.clone() },
            Server { ssl_enabled: true, ..server.clone() },
            Server { connection_uri: Some("postgres://x".into()), ..server.clone() },
            Server { scope_schemas: Some("tenant_*".into()), ..server.clone() },
        ] {
            assert_ne!(connection_identity(&changed), base);
        }
        assert!(!base.contains("secret"));
    }

    #[test]
    fn contract_json_matches_the_front() {
        let event = CatalogEvent {
            server_id: 7,
            database: "app".into(),
            kind: CatalogEventKind::Ready {
                added: vec!["t9".into()],
                removed: vec![],
                changed: vec!["t2".into()],
                fetched_at: Some(5),
            },
        };
        assert_eq!(
            serde_json::to_value(&event).unwrap(),
            serde_json::json!({
                "serverId": 7, "database": "app", "type": "ready",
                "added": ["t9"], "removed": [], "changed": ["t2"], "fetchedAt": 5
            })
        );
        assert_eq!(
            serde_json::to_value(CatalogEvent {
                server_id: 1,
                database: "d".into(),
                kind: CatalogEventKind::Relations { schemas: 3 },
            })
            .unwrap(),
            serde_json::json!({ "serverId": 1, "database": "d", "type": "relations", "schemas": 3 })
        );

        let path: CatalogPathInput =
            serde_json::from_value(serde_json::json!({ "kind": "partitions", "schema": "s", "table": "t" }))
                .unwrap();
        assert_eq!(path.schema(), Some("s"));
        let schemas: CatalogPathInput = serde_json::from_value(serde_json::json!({ "kind": "schemas" })).unwrap();
        assert_eq!(schemas, CatalogPathInput::Schemas);

        let node = crate::catalog::CatalogNode {
            name: "events".into(),
            kind: crate::catalog::NodeKind::MaterializedView,
            child_count: None,
            state: Some(LoadState::Stale),
            drift: Some(crate::catalog::SchemaDrift { missing: 3, extra: 0 }),
        };
        assert_eq!(
            serde_json::to_value(node).unwrap(),
            serde_json::json!({
                "name": "events",
                "kind": "materializedView",
                "childCount": null,
                "state": "stale",
                "drift": { "missing": 3, "extra": 0 }
            })
        );
        let shape: CatalogPathInput =
            serde_json::from_value(serde_json::json!({ "kind": "shape", "key": "other" })).unwrap();
        assert_eq!(shape.schema(), None);
    }
}
