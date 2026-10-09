//! Serviço do catálogo e custo fixo por comando (Fase 3), contra a fixture de
//! `perf/catalog/` (porta 55432 — nunca um banco cadastrado no app).
//!
//! ```text
//! cargo test --release --lib perf::service -- --ignored --nocapture --test-threads=1
//! ```

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use serde_json::json;

use crate::adapters::create_catalog_source;
use crate::catalog::CatalogPath;
use crate::models::{CatalogEvent, CatalogEventKind};
use crate::services::{CatalogService, ConnectionService, SourceOpener};
use crate::storage::vault;

use super::fixture::{env_u64, median_of, pg_client, pg_server, Net, Report, PG_PORT};

const TENANTS_DB: &str = "tenants";
const SERVER_ID: i64 = -1;

pub(super) type Events = Arc<Mutex<Vec<(Instant, CatalogEvent)>>>;

fn opener(port: u16) -> SourceOpener {
    let server = pg_server(port);
    Arc::new(move || {
        let server = server.clone();
        Box::pin(async move { create_catalog_source(&server, TENANTS_DB).await })
    })
}

pub(super) struct TempDir(pub(super) PathBuf);

impl TempDir {
    pub(super) fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("octapus-perf-service-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        Self(dir)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(super) fn service_with_events(dir: &TempDir) -> (CatalogService, Events) {
    vault::init_for_tests();
    let service = CatalogService::default();
    let events: Events = Arc::default();
    let sink = Arc::clone(&events);
    service.init(
        dir.0.clone(),
        Arc::new(move |event| sink.lock().push((Instant::now(), event))),
    );
    (service, events)
}

pub(super) fn first(events: &Events, wanted: &str) -> Option<Instant> {
    events.lock().iter().find_map(|(at, event)| {
        let kind = match event.kind {
            CatalogEventKind::Syncing => "syncing",
            CatalogEventKind::Schemas { .. } => "schemas",
            CatalogEventKind::Relations { .. } => "relations",
            CatalogEventKind::Ready { .. } => "ready",
            CatalogEventKind::Error { .. } => "error",
            CatalogEventKind::Cancelled => "cancelled",
        };
        (kind == wanted).then_some(*at)
    })
}

pub(super) async fn wait_for(events: &Events, wanted: &str) -> Instant {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(at) = first(events, wanted) {
            return at;
        }
        assert!(first(events, "error").is_none(), "erro: {:?}", events.lock().last());
        assert!(Instant::now() < deadline, "esperando {wanted}");
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
}

async fn expected_schemas() -> usize {
    let admin = pg_client(PG_PORT, TENANTS_DB).await;
    let count: i64 = admin
        .query_one(
            "SELECT count(*) FROM pg_namespace
             WHERE nspname <> 'information_schema' AND nspname !~ '^pg_'",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    count as usize
}

#[tokio::test]
#[ignore = "perf: requer perf/catalog (seed.sh up + pg-tenants)"]
async fn e2e_service_end_to_end() {
    let dir = TempDir::new("e2e");
    let (service, events) = service_with_events(&dir);
    let schemas = expected_schemas().await;

    let entry = service.open(SERVER_ID, TENANTS_DB, "perf".into(), opener(PG_PORT)).await;
    assert!(service.ensure_fresh(&entry, false));
    wait_for(&events, "ready").await;
    let order: Vec<_> = ["syncing", "schemas", "relations", "ready"]
        .iter()
        .map(|kind| first(&events, kind).unwrap())
        .collect();
    assert!(order.windows(2).all(|pair| pair[0] <= pair[1]), "eventos fora de ordem");

    let status = service.status(&entry);
    assert_eq!(status.stats.schemas, schemas);
    assert_eq!(status.stats.loaded, schemas);
    assert!(status.server_version.unwrap().starts_with("PostgreSQL"));

    let page = entry
        .read()
        .children(&CatalogPath::Schemas, Some("tenant_0004"), 0, 50)
        .unwrap();
    assert!(page.items.iter().all(|node| node.name.starts_with("tenant_0004")));

    // Catálogo frio (sem sincronizar): abrir um schema carrega só ele
    let cold = CatalogService::default();
    let cold_entry = cold.open(SERVER_ID, TENANTS_DB, "perf".into(), opener(PG_PORT)).await;
    cold.ensure_schemas(&cold_entry, &["tenant_00042".into()]).await.unwrap();
    let tables = cold_entry
        .read()
        .children(&CatalogPath::Schema("tenant_00042".into()), None, 0, 500)
        .unwrap();
    assert_eq!(tables.total, 150);
    assert_eq!(cold.status(&cold_entry).stats.loaded, 1);

    // Refresh de um schema depois de um DDL
    let admin = pg_client(PG_PORT, TENANTS_DB).await;
    admin
        .batch_execute("CREATE TABLE tenant_00007.svc_probe (id int)")
        .await
        .unwrap();
    service.refresh_schema(&entry, "tenant_00007").await.unwrap();
    let names = |entry: &crate::services::CatalogEntry| {
        entry
            .read()
            .children(&CatalogPath::Schema("tenant_00007".into()), None, 0, 500)
            .unwrap()
            .items
            .into_iter()
            .map(|node| node.name)
            .collect::<Vec<_>>()
    };
    assert!(names(&entry).contains(&"svc_probe".to_string()));
    admin.batch_execute("DROP TABLE tenant_00007.svc_probe").await.unwrap();
    service.refresh_schema(&entry, "tenant_00007").await.unwrap();
    assert!(!names(&entry).contains(&"svc_probe".to_string()));

    assert!(service
        .relation_size(&entry, "tenant_00001", "orders")
        .await
        .unwrap()
        .is_some());
    let hits = service.search_all("orders", 10);
    assert_eq!(hits[0].hit.name, "orders");
    assert!(hits[0].hit.schemas.as_ref().unwrap().total > 1);

    // Reabrir: vem do disco, pronto antes de ir ao banco
    {
        let (again, _) = service_with_events(&dir);
        let reopened = again.open(SERVER_ID, TENANTS_DB, "perf".into(), opener(PG_PORT)).await;
        let status = again.status(&reopened);
        assert!(status.from_disk);
        assert_eq!(status.stats.loaded, schemas);
    }

    service.forget_server(SERVER_ID);
    assert!(service.get(SERVER_ID, TENANTS_DB).is_none());
    assert_eq!(std::fs::read_dir(&dir.0).unwrap().count(), 0);
}

pub(super) fn p95(mut samples: Vec<Duration>) -> Duration {
    samples.sort();
    samples[(samples.len() * 95 / 100).min(samples.len() - 1)]
}

/// O caminho do front pelo serviço: camada 0 e prontidão medidas pelos
/// eventos, reabertura do disco, e consultas da memória já serializadas em
/// JSON (aproximação do IPC — o app não roda nestas medições).
#[tokio::test]
#[ignore = "perf: requer perf/catalog (seed.sh up + pg-tenants)"]
async fn perf_service_layers() {
    let net = Net::from_env(PG_PORT).await;
    let report = Report::new("service", &net);
    let port = net.port(PG_PORT);
    let dir = TempDir::new("perf");

    let (service, events) = service_with_events(&dir);
    let start = Instant::now();
    let entry = service.open(SERVER_ID, TENANTS_DB, "perf".into(), opener(port)).await;
    service.ensure_fresh(&entry, false);
    let layer0 = wait_for(&events, "schemas").await - start;
    let ready = wait_for(&events, "ready").await - start;
    report.record(
        "abrir sem cache: camada 0 (evento schemas)",
        Some(layer0),
        json!({ "inclui": "conexão dedicada nova" }),
    );
    report.record("abrir sem cache: pronto (evento ready)", Some(ready), json!({}));

    let (reopened_service, _) = service_with_events(&dir);
    let start = Instant::now();
    let reopened = reopened_service
        .open(SERVER_ID, TENANTS_DB, "perf".into(), opener(port))
        .await;
    let page = reopened
        .read()
        .children(&CatalogPath::Schemas, None, 0, 200)
        .unwrap();
    let from_disk = start.elapsed();
    report.record(
        "abrir com cache em disco: primeira janela da árvore",
        Some(from_disk),
        json!({ "schemas": page.total }),
    );

    let schemas: Vec<String> = entry
        .read()
        .children(&CatalogPath::Schemas, None, 0, usize::MAX)
        .unwrap()
        .items
        .into_iter()
        .map(|node| node.name)
        .collect();

    let mut children = Vec::new();
    let mut bytes = 0;
    for i in 0..2_000 {
        let start = Instant::now();
        let page = entry
            .read()
            .children(&CatalogPath::Schemas, None, (i * 37) % schemas.len(), 200)
            .unwrap();
        bytes = serde_json::to_vec(&page).unwrap().len();
        children.push(start.elapsed());
    }
    report.record(
        "children (janela de 200) + JSON, p95",
        Some(p95(children)),
        json!({ "json_bytes": bytes }),
    );

    let mut expand = Vec::new();
    for i in 0..2_000 {
        let schema = &schemas[(i * 7_919) % schemas.len()];
        let start = Instant::now();
        let page = entry
            .read()
            .children(&CatalogPath::Schema(schema.clone()), None, 0, 200)
            .unwrap();
        serde_json::to_vec(&page).unwrap();
        expand.push(start.elapsed());
    }
    report.record("expandir schema + JSON, p95", Some(p95(expand)), json!({}));

    let mut search = Vec::new();
    for query in ["ord", "orders", "tenant_0042", "tenant_0042.inv", "zzz"] {
        for _ in 0..30 {
            let start = Instant::now();
            let hits = service.search_all(query, 50);
            serde_json::to_vec(&hits).unwrap();
            search.push(start.elapsed());
        }
    }
    report.record("busca na palette + JSON, p95", Some(p95(search)), json!({}));
}

/// Hipótese H6 depois da Fase 3: o custo fixo de um comando de metadado pelo
/// mesmo caminho dos comandos (`ConnectionService::get_live` + adapter).
/// Rode com OCTAPUS_PERF_NET=remote; `rtt` é a unidade.
#[tokio::test]
#[ignore = "perf: requer perf/catalog (seed.sh up + pg-tenants)"]
async fn perf_command_overhead_after() {
    let net = Net::from_env(PG_PORT).await;
    let report = Report::new("service", &net);
    let port = net.port(PG_PORT);
    const N: usize = 7;
    const PING_AFTER_IDLE: Duration = Duration::from_secs(30);

    let raw = pg_client(port, TENANTS_DB).await;
    raw.simple_query("SELECT 1").await.unwrap();
    let rtt = median_of(N, || raw.simple_query("SELECT 1")).await;
    let unit = rtt.as_secs_f64().max(f64::EPSILON);
    let rtts = |elapsed: Duration| (elapsed.as_secs_f64() / unit * 10.0).round() / 10.0;

    let connections = ConnectionService::new();
    let server = pg_server(port);
    let adapter = connections.get_or_connect(&server, TENANTS_DB).await.unwrap();
    // Aquece: cria a conexão do pool e prepara os statements
    adapter.list_databases().await.unwrap();
    adapter.list_columns("tenant_00001", "orders").await.unwrap();

    let command = |kind: &'static str| {
        let connections = &connections;
        async move {
            let adapter = connections
                .get_live(SERVER_ID, TENANTS_DB, PING_AFTER_IDLE)
                .await
                .expect("adapter em cache");
            match kind {
                "databases" => adapter.list_databases().await.map(|_| ()),
                _ => adapter.list_columns("tenant_00001", "orders").await.map(|_| ()),
            }
        }
    };

    report.record("overhead depois: rtt (simple_query)", Some(rtt), json!({ "em_rtts": 1.0 }));
    for (case, kind) in [
        ("overhead depois: list_databases (comando completo)", "databases"),
        ("overhead depois: list_columns (comando completo)", "columns"),
    ] {
        let elapsed = median_of(N, || command(kind)).await;
        report.record(case, Some(elapsed), json!({ "em_rtts": rtts(elapsed) }));
    }

    let ping = median_of(N, || adapter.test_connection()).await;
    report.record(
        "overhead depois: test_connection (ping)",
        Some(ping),
        json!({ "em_rtts": rtts(ping) }),
    );

    // Depois de ociosidade: ping + validação da conexão do pool
    if env_u64("OCTAPUS_PERF_IDLE", 1) == 1 {
        tokio::time::sleep(PING_AFTER_IDLE + Duration::from_secs(1)).await;
        let start = Instant::now();
        command("columns").await.unwrap();
        let elapsed = start.elapsed();
        report.record(
            "overhead depois: list_columns após 31 s parado",
            Some(elapsed),
            json!({ "em_rtts": rtts(elapsed) }),
        );
    }
}
