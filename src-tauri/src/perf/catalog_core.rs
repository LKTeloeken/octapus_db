//! Microbenchmarks do núcleo do catálogo (Fase 1), contra as metas de
//! `perf/catalog/BASELINE.md`: o catálogo do caso do usuário (5.000 schemas ×
//! 150 tabelas, com o mesmo drift da fixture) e o pior caso sem tenants
//! (750 mil nomes únicos). Não usa banco — roda em qualquer máquina:
//!
//! ```text
//! cargo test --release --lib perf::catalog_core -- --ignored --nocapture --test-threads=1
//! ```

use std::fs;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::catalog::{
    Catalog, CatalogKey, CatalogPath, CatalogStore, Loaded, RelKind, SchemaHeader, TableRecord,
};
use crate::storage::vault;

use super::alloc_counter::live_bytes;
use super::fixture::Report;

// ── Metas do núcleo (o resto do orçamento fica para IPC e render) ──

/// Expandir um nó cabe em um frame (16 ms) com folga para IPC e render
const CHILDREN_P95: Duration = Duration::from_millis(5);
/// Busca ≤ 30 ms de ponta a ponta; o IPC de 50 resultados custa ~1 ms
const SEARCH_P95: Duration = Duration::from_millis(25);
const RESOLVE_P95: Duration = Duration::from_millis(1);
/// Abrir com cache em disco ≤ 100 ms (multi-tenant)
const LOAD_MULTI_TENANT: Duration = Duration::from_millis(100);
const MEMORY_MULTI_TENANT: usize = 50 * 1024 * 1024;
const MEMORY_UNIQUE: usize = 150 * 1024 * 1024;

const ENTITIES: [&str; 25] = [
    "customers", "orders", "invoices", "payments", "products", "categories", "suppliers",
    "shipments", "addresses", "contacts", "users", "roles", "permissions", "sessions",
    "notifications", "tickets", "comments", "attachments", "tags", "projects", "tasks", "events",
    "webhooks", "settings", "reports",
];
const SUFFIXES: [&str; 6] = ["", "_items", "_history", "_audit", "_settings", "_links"];

/// Mesmo nome que `perf_table_name(t)` do seed SQL (t começa em 1).
fn table_name(t: usize) -> String {
    format!("{}{}", ENTITIES[(t - 1) % 25], SUFFIXES[((t - 1) / 25) % SUFFIXES.len()])
}

fn tenant(i: usize) -> String {
    format!("tenant_{i:05}")
}

/// Como na fixture: tenants múltiplos de 100 sem as 3 últimas tabelas.
fn tenant_tables(i: usize, per: usize) -> Vec<TableRecord> {
    (1..=per)
        .filter(|&t| !(i.is_multiple_of(100) && t > per - 3))
        .map(|t| TableRecord::new(table_name(t), RelKind::Table))
        .collect()
}

fn shared_schemas() -> Vec<(String, Vec<TableRecord>)> {
    let public = ["plans", "tenant_registry", "schema_migrations"]
        .into_iter()
        .map(|name| TableRecord::new(name, RelKind::Table))
        .collect();
    let mut events = vec![TableRecord::new("event_log", RelKind::Partitioned)];
    events.extend((0..300).map(|p| TableRecord::partition(format!("event_log_p{p:04}"), "event_log")));
    vec![("public".into(), public), ("events".into(), events)]
}

#[derive(Clone, Copy)]
enum Ingest {
    /// Um `set_tables` por schema (o fallback em massa)
    Bulk,
    /// Um representante por formato e `share_shape` para o resto
    ShapeFirst,
}

fn multi_tenant(tenants: usize, per: usize, ingest: Ingest) -> Catalog {
    let mut catalog = Catalog::new();
    catalog.set_schemas(
        (1..=tenants)
            .map(tenant)
            .chain(["public".to_string(), "events".to_string()])
            .map(|name| SchemaHeader {
                name,
                fingerprint: Some(1),
            }),
    );
    for (name, tables) in shared_schemas() {
        catalog.set_tables(&name, tables, Some(1));
    }

    match ingest {
        Ingest::Bulk => {
            for i in 1..=tenants {
                catalog.set_tables(&tenant(i), tenant_tables(i, per), Some(1));
            }
        }
        Ingest::ShapeFirst => {
            catalog.set_tables(&tenant(1), tenant_tables(1, per), Some(1));
            catalog.set_tables(&tenant(100), tenant_tables(100, per), Some(1));
            for i in (2..=tenants).filter(|&i| i != 100) {
                let like = if i.is_multiple_of(100) { tenant(100) } else { tenant(1) };
                assert!(catalog.share_shape(&tenant(i), &like, Some(1)));
            }
        }
    }
    catalog
}

/// Pior caso: nenhum nome se repete, então formatos não economizam nada.
fn unique_names(schemas: usize, per: usize) -> Catalog {
    let mut catalog = Catalog::new();
    for i in 1..=schemas {
        catalog.set_tables(
            &tenant(i),
            (1..=per)
                .map(|t| TableRecord::new(format!("{}_{i}_{t}", table_name(t)), RelKind::Table))
                .collect(),
            Some(1),
        );
    }
    catalog
}

struct Timing {
    p50: Duration,
    p95: Duration,
    max: Duration,
}

fn sample(runs: usize, mut run: impl FnMut(usize)) -> (Timing, Vec<Duration>) {
    let mut samples: Vec<Duration> = (0..runs)
        .map(|i| {
            let start = Instant::now();
            run(i);
            start.elapsed()
        })
        .collect();
    let raw = samples.clone();
    samples.sort();
    let timing = Timing {
        p50: samples[runs / 2],
        p95: samples[(runs * 95 / 100).min(runs - 1)],
        max: samples[runs - 1],
    };
    (timing, raw)
}

fn ms(duration: Duration) -> f64 {
    (duration.as_secs_f64() * 1000.0 * 1000.0).round() / 1000.0
}

fn record_timing(report: &Report, case: &str, timing: &Timing, extra: Value) {
    let mut fields = json!({
        "p50_ms": ms(timing.p50),
        "p95_ms": ms(timing.p95),
        "max_ms": ms(timing.max),
    });
    if let (Some(fields), Value::Object(extra)) = (fields.as_object_mut(), extra) {
        fields.extend(extra);
    }
    report.record(case, Some(timing.p95), fields);
}

/// Constrói medindo tempo e heap de verdade (os temporários da entrada já
/// foram liberados quando a medição fecha).
fn build_measured(report: &Report, case: &str, build: impl FnOnce() -> Catalog) -> (Catalog, usize) {
    let before = live_bytes();
    let start = Instant::now();
    let catalog = build();
    let elapsed = start.elapsed();
    let heap = live_bytes().saturating_sub(before);

    let stats = catalog.stats();
    report.record(
        case,
        Some(elapsed),
        json!({
            "schemas": stats.schemas,
            "shapes": stats.shapes,
            "distinct_names": stats.distinct_names,
            "relations": stats.relations,
            "heap_mb": heap as f64 / 1e6,
            "approx_heap_mb": stats.approx_heap_bytes as f64 / 1e6,
        }),
    );
    (catalog, heap)
}

struct Workload<'a> {
    report: &'a Report,
    schemas: Vec<String>,
    filter: &'a str,
    queries: &'a [&'a str],
    resolve: &'a [&'a str],
}

/// Roda as consultas do front e confere as metas de tempo.
fn exercise(catalog: &Catalog, work: Workload) {
    let report = work.report;
    let schemas = &work.schemas;
    let total = catalog.stats().schemas;
    let max_offset = total.saturating_sub(200).max(1);

    let (timing, _) = sample(2_000, |i| {
        let page = catalog
            .children(&CatalogPath::Schemas, None, (i * 7_919) % max_offset, 200)
            .unwrap();
        assert_eq!(page.total, total);
    });
    record_timing(report, "children: schemas (janela de 200)", &timing, json!({}));
    assert!(timing.p95 <= CHILDREN_P95, "children p95 {:?}", timing.p95);

    let (timing, _) = sample(500, |_| {
        catalog
            .children(&CatalogPath::Schemas, Some(work.filter), 0, 200)
            .unwrap();
    });
    record_timing(
        report,
        "children: schemas com filtro",
        &timing,
        json!({ "filter": work.filter }),
    );
    assert!(timing.p95 <= CHILDREN_P95, "children com filtro p95 {:?}", timing.p95);

    let (timing, _) = sample(2_000, |i| {
        let schema = &schemas[(i * 104_729) % schemas.len()];
        catalog
            .children(&CatalogPath::Schema(schema.clone()), None, 0, 200)
            .unwrap();
    });
    record_timing(report, "children: relações de um schema", &timing, json!({}));
    assert!(timing.p95 <= CHILDREN_P95, "children de schema p95 {:?}", timing.p95);

    let mut all_runs = Vec::new();
    for &query in work.queries {
        let mut hits = 0;
        let (timing, raw) = sample(30, |_| hits = catalog.search(query, 50).len());
        all_runs.extend(raw);
        record_timing(
            report,
            &format!("search: \"{query}\""),
            &timing,
            json!({ "hits": hits }),
        );
    }
    all_runs.sort();
    let overall_p95 = all_runs[(all_runs.len() * 95 / 100).min(all_runs.len() - 1)];
    report.record(
        "search: p95 de todas as buscas",
        Some(overall_p95),
        json!({ "runs": all_runs.len() }),
    );
    assert!(overall_p95 <= SEARCH_P95, "search p95 {overall_p95:?}");

    let (timing, _) = sample(2_000, |i| {
        let table = work.resolve[i % work.resolve.len()];
        let schema = &schemas[(i * 7_919) % schemas.len()];
        catalog.resolve(table, &[schema.as_str(), "public"]);
    });
    record_timing(report, "resolve: com search_path", &timing, json!({}));
    assert!(timing.p95 <= RESOLVE_P95, "resolve p95 {:?}", timing.p95);

    let (timing, _) = sample(500, |i| {
        catalog.resolve(work.resolve[i % work.resolve.len()], &[]);
    });
    record_timing(report, "resolve: sem search_path (ambíguo)", &timing, json!({}));
    assert!(timing.p95 <= RESOLVE_P95, "resolve ambíguo p95 {:?}", timing.p95);

    let (timing, _) = sample(2_000, |i| {
        let schema = &schemas[(i * 7_919) % schemas.len()];
        catalog.complete_tables(schema, "ord", 50).unwrap();
    });
    record_timing(report, "complete_tables: prefixo num schema", &timing, json!({}));

    let start = Instant::now();
    let drift = catalog.drift();
    report.record(
        "drift",
        Some(start.elapsed()),
        json!({
            "dominant_tables": drift.dominant.map(|d| d.tables),
            "divergent_groups": drift.divergent.len(),
        }),
    );
}

/// Grava e lê pelo `CatalogStore` (com fsync), num diretório descartável.
fn persist_round_trip(report: &Report, catalog: &Catalog, load_budget: Option<Duration>) {
    vault::init_for_tests();
    let dir = std::env::temp_dir().join(format!("octapus-perf-catalog-{}", std::process::id()));
    let store = CatalogStore::new(&dir);
    let key = CatalogKey {
        server_id: 1,
        database: "tenants",
        identity: "perf",
    };

    let start = Instant::now();
    store.save(&key, catalog).unwrap();
    let saved = start.elapsed();
    let bytes = fs::metadata(store.path(&key)).unwrap().len();
    report.record(
        "persist: save (encode + lz4 + AES-GCM + fsync)",
        Some(saved),
        json!({ "file_kb": bytes / 1024 }),
    );

    let start = Instant::now();
    let loaded = store.load(&key).unwrap();
    let elapsed = start.elapsed();
    let Loaded::Hit(loaded) = loaded else {
        panic!("o catálogo salvo não voltou");
    };
    report.record("persist: load", Some(elapsed), json!({}));
    assert_eq!(loaded.stats().relations, catalog.stats().relations);
    if let Some(budget) = load_budget {
        assert!(elapsed <= budget, "load {elapsed:?}");
    }

    let _ = fs::remove_dir_all(dir);
}

#[test]
#[ignore = "perf: microbenchmark do núcleo do catálogo (rodar em --release)"]
fn perf_catalog_core_multi_tenant() {
    let bulk = Report::offline("catalog-core", "L-bulk");
    let (catalog, _) = build_measured(&bulk, "build: em massa (5.000 × set_tables)", || {
        multi_tenant(5_000, 150, Ingest::Bulk)
    });
    drop(catalog);

    let report = Report::offline("catalog-core", "L");
    let (catalog, heap) = build_measured(&report, "build: shape-first (2 formatos + share_shape)", || {
        multi_tenant(5_000, 150, Ingest::ShapeFirst)
    });
    assert!(heap <= MEMORY_MULTI_TENANT, "heap {heap}");
    assert_eq!(catalog.stats().relations, 750_000 - 50 * 3 + 3 + 301);

    exercise(
        &catalog,
        Workload {
            report: &report,
            schemas: (1..=5_000).map(tenant).collect(),
            filter: "tenant_04",
            queries: &[
                "ord",
                "orders",
                "invoices_hist",
                "tenant_0042",
                "tenant_04213",
                "tenant_0042.ord",
                "t.inv",
                "x",
                "zzzz",
                "settings_links",
            ],
            resolve: &["orders", "INVOICES", "reports_links", "event_log", "missing"],
        },
    );
    persist_round_trip(&report, &catalog, Some(LOAD_MULTI_TENANT));
}

#[test]
#[ignore = "perf: microbenchmark do núcleo do catálogo (rodar em --release)"]
fn perf_catalog_core_unique_names() {
    let report = Report::offline("catalog-core", "U");
    let (catalog, heap) = build_measured(&report, "build: 1.500 × 500 nomes únicos", || {
        unique_names(1_500, 500)
    });
    assert!(heap <= MEMORY_UNIQUE, "heap {heap}");

    exercise(
        &catalog,
        Workload {
            report: &report,
            schemas: (1..=1_500).map(tenant).collect(),
            filter: "tenant_01",
            queries: &[
                "ord",
                "orders_12",
                "invoices_history_1499",
                "tenant_0042",
                "tenant_0042.ord",
                "x",
                "zzzz",
            ],
            resolve: &["orders_12_2", "INVOICES_7_3", "missing"],
        },
    );
    persist_round_trip(&report, &catalog, None);
}

/// Fase 6: agrupar por formato e o aviso de drift em cada schema da janela,
/// no caso do usuário e num banco em que quase cada tenant diverge de um jeito
/// (o pior caso para os grupos e para o aviso).
#[test]
#[ignore = "perf: microbenchmark do núcleo do catálogo (rodar em --release)"]
fn perf_catalog_core_shapes() {
    use crate::catalog::NameScope;

    let report = Report::offline("catalog-core", "L-shapes");
    let catalog = multi_tenant(5_000, 150, Ingest::ShapeFirst);
    let groups = catalog.shape_groups();
    assert_eq!(groups.len(), 3);

    let (timing, _) = sample(200, |_| {
        std::hint::black_box(catalog.shape_groups());
    });
    record_timing(&report, "shape_groups", &timing, json!({ "groups": groups.len() }));

    let (timing, _) = sample(200, |i| {
        let offset = (i * 500) % 5_000;
        std::hint::black_box(catalog.children(&CatalogPath::Schemas, None, offset, 500));
    });
    assert!(timing.p95 <= CHILDREN_P95, "{:?}", timing.p95);
    record_timing(&report, "children: janela de 500 schemas com drift", &timing, json!({}));

    for group in &groups {
        let path = CatalogPath::ShapeSchemas(group.key.clone());
        let (timing, _) = sample(200, |_| {
            std::hint::black_box(catalog.children(&path, None, 0, 500));
        });
        assert!(timing.p95 <= CHILDREN_P95, "{:?}", timing.p95);
        record_timing(
            &report,
            &format!("children: grupo {:?} (janela de 500)", group.role),
            &timing,
            json!({ "schemas": group.schemas }),
        );
    }

    // Pior caso: 2.000 tenants no molde e 3.000 em 1.500 variações de 2
    // tenants cada (cada par sem uma tabela diferente)
    let report = Report::offline("catalog-core", "L-drift-spread");
    let mut catalog = Catalog::new();
    for i in 1..=2_000 {
        catalog.set_tables(&tenant(i), tenant_tables(1, 150), Some(1));
    }
    for i in 2_001..=5_000 {
        // Cada par sem uma tabela; a partir do 150º par, também com uma a mais
        let pair = (i - 2_001) / 2;
        let dropped = table_name(pair % 150 + 1);
        let tables: Vec<TableRecord> = tenant_tables(1, 150)
            .into_iter()
            .filter(|table| table.name != dropped)
            .chain((pair >= 150).then(|| TableRecord::new(format!("extra_{pair}"), RelKind::Table)))
            .collect();
        catalog.set_tables(&tenant(i), tables, Some(1));
    }
    let groups = catalog.shape_groups();
    let (timing, _) = sample(50, |_| {
        std::hint::black_box(catalog.shape_groups());
    });
    record_timing(
        &report,
        "shape_groups (1.500 variações, 50 listadas)",
        &timing,
        json!({ "groups": groups.len(), "shapes": catalog.stats().shapes }),
    );
    let (timing, _) = sample(100, |i| {
        let offset = 2_000 + (i * 500) % 3_000;
        std::hint::black_box(catalog.children(&CatalogPath::Schemas, None, offset, 500));
    });
    record_timing(&report, "children: janela de 500 schemas, 250 formatos distintos", &timing, json!({}));
    let (timing, _) = sample(50, |_| {
        std::hint::black_box(catalog.children(&CatalogPath::ShapeSchemas("other".into()), None, 0, 500));
    });
    record_timing(&report, "children: grupo \"outros\" (janela de 500)", &timing, json!({}));

    // Escopo salvo: 5.000 nomes contra padrões de incluir e excluir
    let scope = NameScope::parse(Some("public, tenant_0*, !tenant_00*5, !*_test"));
    let names: Vec<String> = (1..=5_000).map(tenant).collect();
    let (timing, _) = sample(200, |_| {
        std::hint::black_box(names.iter().filter(|name| scope.allows(name)).count());
    });
    record_timing(&report, "escopo: 5.000 schemas contra 4 padrões", &timing, json!({}));
}
