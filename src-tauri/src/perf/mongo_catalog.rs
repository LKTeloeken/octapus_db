//! Catálogo do Mongo (Fase 5) contra a fixture de `perf/catalog/` (porta
//! 57017 — nunca um banco cadastrado no app): o database `tenants` (5.000
//! coleções) e os `tenantdb_NNN` (um database por cliente).
//!
//! ```text
//! cargo test --release --lib perf::mongo_catalog -- --ignored --nocapture --test-threads=1
//! ```

use std::sync::Arc;
use std::time::Duration;

use mongodb::bson::doc;
use serde_json::json;

use crate::adapters::mongo::introspect::MongoSource;
use crate::adapters::mongo::MongoAdapter;
use crate::adapters::{create_catalog_source, DatabaseAdapter};
use crate::catalog::{Catalog, CatalogPath, CatalogSource, NodeKind, FLAT_SCHEMA};
use crate::services::SourceOpener;

use super::fixture::{median_of, mongo_server, timed, Net, Report, MONGO_PORT};
use super::mongo_baseline::{raw_client, TENANTS_DB};
use super::service::{first, p95, service_with_events, wait_for, TempDir};

const SERVER_ID: i64 = -1;
const PROBE: &str = "zz_f5_probe";
const PROBE_VIEW: &str = "zz_f5_view";

fn opener(port: u16, database: &str) -> SourceOpener {
    let server = mongo_server(port);
    let database = database.to_string();
    Arc::new(move || {
        let (server, database) = (server.clone(), database.clone());
        Box::pin(async move { create_catalog_source(&server, &database).await })
    })
}

fn flat_names(catalog: &parking_lot::RwLock<Catalog>) -> Vec<String> {
    catalog
        .read()
        .children(&CatalogPath::Schema(FLAT_SCHEMA.into()), None, 0, 10_000)
        .unwrap()
        .items
        .into_iter()
        .map(|node| node.name)
        .collect()
}

#[tokio::test]
#[ignore = "perf: requer perf/catalog (seed.sh up + mongo)"]
async fn e2e_mongo_source() {
    let client = raw_client(MONGO_PORT).await;
    let db = client.database(TENANTS_DB);
    let _ = db.collection::<mongodb::bson::Document>(PROBE).drop().await;
    let _ = db.collection::<mongodb::bson::Document>(PROBE_VIEW).drop().await;
    let mut expected = db.list_collection_names().await.unwrap();
    expected.sort();

    let mut source = MongoSource::connect(&mongo_server(MONGO_PORT), TENANTS_DB).await.unwrap();
    assert!(source.server_version().starts_with("MongoDB 7"));

    let catalog = parking_lot::RwLock::new(Catalog::new());
    let mut events = Vec::new();
    let report = source.sync(&catalog, &mut |event| events.push(event)).await.unwrap();
    assert_eq!(report.fetched, 1);
    assert_eq!(events.len(), 2);
    assert_eq!(flat_names(&catalog), expected);

    // Sem mudança, revalidar não recarrega
    assert_eq!(source.sync(&catalog, &mut |_| {}).await.unwrap().fetched, 0);

    // Coleção e view novas: o fingerprint muda e elas aparecem com o tipo certo
    db.create_collection(PROBE).await.unwrap();
    db.run_command(doc! { "create": PROBE_VIEW, "viewOn": PROBE, "pipeline": [] })
        .await
        .unwrap();
    let report = source.sync(&catalog, &mut |_| {}).await.unwrap();
    assert_eq!(report.diff.changed, vec![FLAT_SCHEMA.to_string()]);
    let view = catalog
        .read()
        .children(&CatalogPath::Schema(FLAT_SCHEMA.into()), Some(PROBE_VIEW), 0, 10)
        .unwrap()
        .items;
    assert_eq!(view[0].kind, NodeKind::View);
    db.collection::<mongodb::bson::Document>(PROBE_VIEW).drop().await.unwrap();
    db.collection::<mongodb::bson::Document>(PROBE).drop().await.unwrap();
    source.sync(&catalog, &mut |_| {}).await.unwrap();
    // A view deixa o `system.views` para trás: o catálogo mostra o que o banco lista
    let mut expected = db.list_collection_names().await.unwrap();
    expected.sort();
    assert_eq!(flat_names(&catalog), expected);

    // Tamanho só da coleção pedida; busca e resolução da memória
    let sample = expected.iter().find(|name| name.ends_with("_customers")).unwrap();
    assert!(source.relation_size(FLAT_SCHEMA, sample).await.unwrap().is_some());
    let hits = catalog.read().search("customers", 10);
    assert!(hits.iter().all(|hit| hit.name.contains("customers")));
    let resolved = catalog.read().resolve(sample, &[FLAT_SCHEMA]);
    assert!(matches!(resolved, crate::catalog::Resolution::Found { ref schema, .. } if schema.is_empty()));

    // Cancelar no meio não deixa o catálogo pela metade
    let canceller = source.canceller();
    let fresh = parking_lot::RwLock::new(Catalog::new());
    let mut ignore = |_| {};
    let (result, _) = tokio::join!(source.sync(&fresh, &mut ignore), async {
        tokio::task::yield_now().await;
        canceller.cancel().await.unwrap();
    });
    match result {
        Err(crate::error::Error::Cancelled) => assert_eq!(fresh.read().stats().schemas, 0),
        // Rápido demais para o cancelamento pegar: completo, não parcial
        Ok(_) => assert_eq!(flat_names(&fresh), expected),
        Err(other) => panic!("{other}"),
    }

    // Nível do servidor: só nomes, inclusive os bancos de tenant
    let adapter = MongoAdapter::new(&mongo_server(MONGO_PORT), "admin").await.unwrap();
    let databases = adapter.list_databases().await.unwrap();
    assert!(databases.iter().filter(|db| db.name.starts_with("tenantdb_")).count() >= 300);
    assert!(databases.iter().all(|db| db.size_bytes.is_none()));
}

/// Abrir o database pelo serviço (conexão nova incluída), revalidar, e o
/// nível do servidor — os números que a baseline tinha em 49 s.
#[tokio::test]
#[ignore = "perf: requer perf/catalog (seed.sh up + mongo)"]
async fn perf_mongo_catalog() {
    let net = Net::from_env(MONGO_PORT).await;
    let report = Report::new("mongo", &net);
    let port = net.port(MONGO_PORT);

    // Abrir sem cache, pelo serviço: o tempo até cada evento
    let dir = TempDir::new("mongo");
    let (service, events) = service_with_events(&dir);
    let started = std::time::Instant::now();
    let entry = service.open(SERVER_ID, TENANTS_DB, "perf".into(), opener(port, TENANTS_DB)).await;
    service.ensure_fresh(&entry, false);
    wait_for(&events, "ready").await;
    let schemas_at = first(&events, "schemas").unwrap() - started;
    let ready_at = first(&events, "ready").unwrap() - started;
    let collections = service.status(&entry).stats.relations;
    report.record(
        "catálogo: abrir sem cache → coleções visíveis (conexão nova)",
        Some(schemas_at),
        json!({ "collections": collections }),
    );
    report.record("catálogo: abrir sem cache → pronto", Some(ready_at), json!({}));

    // Revalidar com a fonte já aberta (o que acontece ao reabrir o database)
    let mut source = MongoSource::connect(&mongo_server(port), TENANTS_DB).await.unwrap();
    let catalog = parking_lot::RwLock::new(Catalog::new());
    source.sync(&catalog, &mut |_| {}).await.unwrap();
    let mut samples = Vec::new();
    for _ in 0..5 {
        let (report, elapsed) = timed(source.sync(&catalog, &mut |_| {})).await;
        assert_eq!(report.unwrap().fetched, 0);
        samples.push(elapsed);
    }
    samples.sort();
    report.record("catálogo: revalidar (listCollections nameOnly)", Some(samples[2]), json!({}));

    let sample = flat_names(&catalog).into_iter().find(|name| name.ends_with("_orders")).unwrap();
    let (size, elapsed) = timed(source.relation_size(FLAT_SCHEMA, &sample)).await;
    report.record(
        "catálogo: tamanho da coleção aberta ($collStats de uma)",
        Some(elapsed),
        json!({ "bytes": size.unwrap() }),
    );

    // Consultas da memória, já em JSON (aproximação do IPC)
    let mut window = Vec::new();
    let mut search = Vec::new();
    for _ in 0..50 {
        let (_, elapsed) = timed(async {
            let page = entry
                .read()
                .children(&CatalogPath::Schema(FLAT_SCHEMA.into()), None, 0, 500)
                .unwrap();
            serde_json::to_vec(&page).unwrap()
        })
        .await;
        window.push(elapsed);
        let (_, elapsed) = timed(async { serde_json::to_vec(&service.search_all("orders", 50)).unwrap() }).await;
        search.push(elapsed);
    }
    report.record("catálogo: janela de 500 coleções + JSON, p95", Some(p95(window)), json!({}));
    report.record("catálogo: busca da palette + JSON, p95", Some(p95(search)), json!({}));

    // Nível do servidor: o adapter agora pede só os nomes
    let adapter = MongoAdapter::new(&mongo_server(port), "admin").await.unwrap();
    adapter.test_connection().await.unwrap();
    let elapsed = median_of(5, || adapter.list_databases()).await;
    let databases = adapter.list_databases().await.unwrap();
    report.record(
        "servidor: list_databases (nameOnly, authorizedDatabases)",
        Some(elapsed),
        json!({ "databases": databases.len() }),
    );

    // Um database por tenant: abrir um banco pequeno pelo serviço
    let started = std::time::Instant::now();
    let tenant = service.open(SERVER_ID, "tenantdb_042", "perf".into(), opener(port, "tenantdb_042")).await;
    service.ensure_fresh(&tenant, false);
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    while service.status(&tenant).fetched_at.is_none() {
        assert!(std::time::Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    report.record(
        "catálogo: abrir um database de tenant (20 coleções, conexão nova)",
        Some(started.elapsed()),
        json!({ "collections": service.status(&tenant).stats.relations }),
    );
}
