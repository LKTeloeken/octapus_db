//! Testes e medições da introspecção do Postgres (Fase 2), contra a fixture de
//! `perf/catalog/` (porta 55432 — nunca um banco cadastrado no app).
//!
//! ```text
//! # correção (tier S basta)
//! cargo test --release --lib perf::introspect::e2e -- --ignored --nocapture --test-threads=1
//! # desempenho por camada (tier e rede via OCTAPUS_PERF_TIER / OCTAPUS_PERF_NET)
//! cargo test --release --lib perf::introspect::perf -- --ignored --nocapture --test-threads=1
//! ```

use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

use parking_lot::RwLock;
use serde_json::json;

use crate::adapters::postgres::introspect::{IntrospectOptions, Introspector};
use crate::catalog::{Catalog, CatalogPath, LoadState, NodeKind, Strategy, SyncEvent, SyncReport};
use crate::error::Error;

use super::fixture::{env_u64, pg_client, pg_server, Net, Report, PG_PORT};

const TENANTS_DB: &str = "tenants";
const SCRATCH_DB: &str = "catalog_e2e";

/// Todas as relações do catálogo: schema → (nome, tipo, pai).
type Dump = BTreeMap<String, BTreeSet<(String, NodeKind, Option<String>)>>;

fn dump(catalog: &Catalog) -> Dump {
    let mut out = Dump::new();
    let schemas = catalog
        .children(&CatalogPath::Schemas, None, 0, usize::MAX)
        .unwrap();
    for schema in schemas.items {
        let relations = out.entry(schema.name.clone()).or_default();
        let top = catalog
            .children(&CatalogPath::Schema(schema.name.clone()), None, 0, usize::MAX)
            .unwrap();
        for table in top.items {
            if table.kind == NodeKind::Partitioned {
                let partitions = catalog
                    .children(
                        &CatalogPath::Partitions {
                            schema: schema.name.clone(),
                            table: table.name.clone(),
                        },
                        None,
                        0,
                        usize::MAX,
                    )
                    .unwrap();
                for partition in partitions.items {
                    relations.insert((partition.name, partition.kind, Some(table.name.clone())));
                }
            }
            relations.insert((table.name, table.kind, None));
        }
    }
    out
}

async fn connect(port: u16, database: &str, options: IntrospectOptions) -> Introspector {
    Introspector::connect(&pg_server(port), database, options)
        .await
        .expect("conexão dedicada com a fixture")
}

async fn sync(introspector: &mut Introspector, catalog: &RwLock<Catalog>) -> SyncReport {
    introspector.sync(catalog, |_| {}).await.expect("sync")
}

fn ms(duration: Duration) -> f64 {
    (duration.as_secs_f64() * 1000.0 * 10.0).round() / 10.0
}

// ─────────────────────────────────────────────────────────────────────────────
// Correção
// ─────────────────────────────────────────────────────────────────────────────

/// Diferencial: o catálogo novo enxerga o mesmo que a listagem atual, a não
/// ser pelas correções de propósito — tabela particionada (`'p'`) aparece, com
/// as partições dentro dela. E shape-first e em massa dão o mesmo catálogo.
#[tokio::test]
#[ignore = "perf: requer perf/catalog (seed.sh up + pg-tenants 500)"]
async fn e2e_matches_the_current_listing() {
    // No tier L a listagem atual derruba o backend (OOM): com
    // OCTAPUS_PERF_SKIP_FULL=1 o teste compara só shape-first × em massa
    let current = if env_u64("OCTAPUS_PERF_SKIP_FULL", 0) == 1 {
        None
    } else {
        Some(legacy_listing().await)
    };

    let catalog = RwLock::new(Catalog::new());
    let mut introspector = connect(PG_PORT, TENANTS_DB, IntrospectOptions::default()).await;
    let report = sync(&mut introspector, &catalog).await;
    assert_eq!(report.strategy, Strategy::ShapeFirst);
    let new = dump(&catalog.read());

    if let Some(current) = &current {
        compare_with_current(current, &new);
    }

    let events = &new["events"];
    assert!(events.contains(&("event_log".into(), NodeKind::Partitioned, None)));
    assert_eq!(
        events.iter().filter(|(_, _, parent)| parent.as_deref() == Some("event_log")).count(),
        300
    );

    let bulk = RwLock::new(Catalog::new());
    let mut introspector = connect(
        PG_PORT,
        TENANTS_DB,
        IntrospectOptions {
            force_bulk: true,
            ..Default::default()
        },
    )
    .await;
    assert_eq!(sync(&mut introspector, &bulk).await.strategy, Strategy::Bulk);
    assert_eq!(dump(&bulk.read()), new);

    // O drift da fixture: tenants múltiplos de 100 sem as 3 últimas tabelas
    let drift = catalog.read().drift();
    assert_eq!(drift.dominant.unwrap().tables, 150);
    assert_eq!(drift.divergent.len(), 1);
    assert_eq!(drift.divergent[0].missing.len(), 3);
}

/// Fase 6: o escopo salvo na conexão vale nos dois caminhos (shape-first e em
/// massa) e na carga prioritária — o que fica de fora não entra no catálogo.
#[tokio::test]
#[ignore = "perf: requer perf/catalog (seed.sh up + pg-tenants 500)"]
async fn e2e_scope() {
    use crate::catalog::NameScope;

    let scope = || NameScope::parse(Some("public, tenant_0001*, !tenant_00015"));
    let expected: Vec<String> = std::iter::once("public".to_string())
        .chain((10..=19).filter(|i| *i != 15).map(|i| format!("tenant_{i:05}")))
        .collect();

    for force_bulk in [false, true] {
        let mut introspector = connect(
            PG_PORT,
            TENANTS_DB,
            IntrospectOptions {
                scope: scope(),
                force_bulk,
                ..Default::default()
            },
        )
        .await;
        let catalog = RwLock::new(Catalog::new());
        let report = sync(&mut introspector, &catalog).await;
        assert_eq!(report.schemas, expected.len(), "force_bulk={force_bulk}");
        let dumped = dump(&catalog.read());
        assert_eq!(dumped.keys().cloned().collect::<Vec<_>>(), expected);
        assert_eq!(dumped["tenant_00012"].len(), 150);

        // Fora do escopo: a carga prioritária não vai ao banco por ele
        let fresh = RwLock::new(Catalog::new());
        let loaded = introspector
            .load_schemas(&fresh, &["tenant_00042".into(), "tenant_00011".into()])
            .await
            .unwrap();
        assert_eq!(loaded, 1);
        assert!(fresh.read().schema_state("tenant_00042").is_none());
    }
}

/// O que a listagem antiga (`list_schemas_with_tables`, que saiu do adapter)
/// enxergava: schema → relações r/v/m/f, sem o tamanho.
async fn legacy_listing() -> BTreeMap<String, BTreeSet<String>> {
    let client = pg_client(PG_PORT, TENANTS_DB).await;
    let rows = client
        .query(
            "SELECT n.nspname, c.relname FROM pg_namespace n
             LEFT JOIN pg_class c ON c.relnamespace = n.oid AND c.relkind IN ('r', 'v', 'm', 'f')
             WHERE n.nspname NOT IN ('pg_toast', 'pg_catalog', 'information_schema')",
            &[],
        )
        .await
        .unwrap();
    let mut listing: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for row in rows {
        let tables = listing.entry(row.get(0)).or_default();
        if let Some(table) = row.get::<_, Option<String>>(1) {
            tables.insert(table);
        }
    }
    listing
}

/// Antes: só r/v/m/f, partições soltas. Novo, tirando o pai 'p': tem que ser igual.
fn compare_with_current(current: &BTreeMap<String, BTreeSet<String>>, new: &Dump) {
    let current_schemas: BTreeSet<&str> = current
        .keys()
        .map(String::as_str)
        .filter(|name| !name.starts_with("pg_"))
        .collect();
    assert_eq!(current_schemas, new.keys().map(String::as_str).collect());

    for (schema, expected) in current {
        let Some(relations) = new.get(schema) else {
            continue;
        };
        let seen: BTreeSet<String> = relations
            .iter()
            .filter(|(_, kind, _)| *kind != NodeKind::Partitioned)
            .map(|(name, _, _)| name.clone())
            .collect();
        assert_eq!(&seen, expected, "schema {schema}");
    }
}

/// O fingerprint muda com o que muda a estrutura e não com manutenção; uma
/// revalidação só busca o que mudou.
#[tokio::test]
#[ignore = "perf: requer perf/catalog (seed.sh up)"]
async fn e2e_fingerprint_semantics() {
    let admin = pg_client(PG_PORT, "postgres").await;
    admin
        .batch_execute(&format!("DROP DATABASE IF EXISTS {SCRATCH_DB}"))
        .await
        .unwrap();
    admin
        .batch_execute(&format!("CREATE DATABASE {SCRATCH_DB}"))
        .await
        .unwrap();

    let db = pg_client(PG_PORT, SCRATCH_DB).await;
    db.batch_execute(
        "CREATE SCHEMA a; CREATE SCHEMA b; CREATE SCHEMA c;
         CREATE TABLE a.t1 (id int PRIMARY KEY, v text);
         CREATE TABLE a.t2 (id int);
         CREATE TABLE b.t1 (id int PRIMARY KEY, v text);
         CREATE TABLE b.t2 (id int);
         CREATE VIEW a.v1 AS SELECT 1 AS one;",
    )
    .await
    .unwrap();

    let catalog = RwLock::new(Catalog::new());
    let mut introspector = connect(PG_PORT, SCRATCH_DB, IntrospectOptions::default()).await;

    let first = sync(&mut introspector, &catalog).await;
    assert_eq!(first.schemas, 4, "a, b, c e public");
    assert_eq!(catalog.read().pending_schemas(), Vec::<String>::new());

    // Uma sessão com tabela temporária não pode fazer aparecer pg_temp_N
    let temp_session = pg_client(PG_PORT, SCRATCH_DB).await;
    temp_session.batch_execute("CREATE TEMP TABLE scratch (id int)").await.unwrap();

    let expect_changes = |report: &SyncReport, label: &str, changed: &[&str], fetched: usize| {
        assert_eq!(report.diff.changed, changed, "{label}: changed");
        assert_eq!(report.fetched + report.shared, fetched, "{label}: recarregados");
    };

    let report = sync(&mut introspector, &catalog).await;
    expect_changes(&report, "sem mudança", &[], 0);

    // VACUUM não roda num bloco de vários comandos: um por chamada
    for statement in [
        "INSERT INTO a.t1 SELECT g, 'x' FROM generate_series(1, 5000) g",
        "VACUUM ANALYZE a.t1",
        "ANALYZE b.t1",
    ] {
        db.batch_execute(statement).await.unwrap();
    }
    let report = sync(&mut introspector, &catalog).await;
    expect_changes(&report, "VACUUM/ANALYZE", &[], 0);

    db.batch_execute("COMMENT ON TABLE a.t1 IS 'só pg_description'").await.unwrap();
    let report = sync(&mut introspector, &catalog).await;
    expect_changes(&report, "COMMENT", &[], 0);

    for (label, statement, schema) in [
        ("CREATE TABLE", "CREATE TABLE a.t3 (id int)", "a"),
        ("ADD COLUMN", "ALTER TABLE b.t1 ADD COLUMN extra int", "b"),
        ("RENAME", "ALTER TABLE a.t3 RENAME TO t4", "a"),
        ("DROP TABLE", "DROP TABLE a.t4", "a"),
        // GRANT reescreve relacl: falso positivo esperado (só custa uma recarga)
        ("GRANT", "GRANT SELECT ON b.t2 TO PUBLIC", "b"),
    ] {
        db.batch_execute(statement).await.unwrap();
        let report = sync(&mut introspector, &catalog).await;
        expect_changes(&report, label, &[schema], 1);
    }
    assert!(dump(&catalog.read())["a"]
        .iter()
        .all(|(name, _, _)| name != "t3" && name != "t4"));

    db.batch_execute(
        "CREATE TABLE b.events (id int, at date) PARTITION BY RANGE (at);
         CREATE TABLE b.events_2025 PARTITION OF b.events FOR VALUES FROM ('2025-01-01') TO ('2026-01-01');
         CREATE TABLE b.events_2026 PARTITION OF b.events FOR VALUES FROM ('2026-01-01') TO ('2027-01-01');
         CREATE SCHEMA d; CREATE TABLE d.only_one (id int);
         DROP SCHEMA c;",
    )
    .await
    .unwrap();
    let mut events = Vec::new();
    let report = introspector
        .sync(&catalog, |event| events.push(event))
        .await
        .unwrap();
    assert_eq!(report.diff.added, ["d"]);
    assert_eq!(report.diff.removed, ["c"]);
    assert_eq!(report.diff.changed, ["b"]);
    assert!(matches!(events[0], SyncEvent::Schemas(_)), "camada 0 avisada primeiro");

    let b = &dump(&catalog.read())["b"];
    assert!(b.contains(&("events".into(), NodeKind::Partitioned, None)));
    assert!(b.contains(&("events_2025".into(), NodeKind::Table, Some("events".into()))));
    assert!(!dump(&catalog.read()).keys().any(|name| name.starts_with("pg_")));

    // Prioridade: carregar um schema pedido pelo usuário, num catálogo novo
    let fresh = RwLock::new(Catalog::new());
    assert_eq!(introspector.load_schemas(&fresh, &["d".into(), "nope".into()]).await.unwrap(), 1);
    assert_eq!(fresh.read().schema_state("d"), Some(LoadState::Loaded));

    // Tamanho sob demanda
    assert!(introspector.relation_size("a", "t1").await.unwrap().unwrap() > 0);
    assert_eq!(introspector.relation_size("a", "nope").await.unwrap(), None);

    drop(introspector);
    drop(temp_session);
    drop(db);
    admin
        .batch_execute(&format!("DROP DATABASE {SCRATCH_DB} WITH (FORCE)"))
        .await
        .unwrap();
}

/// Uma migração segurando ACCESS EXCLUSIVE não trava a introspecção (só lê o
/// catálogo); o tamanho daquela tabela falha rápido pelo `lock_timeout`.
#[tokio::test]
#[ignore = "perf: requer perf/catalog (seed.sh up + pg-tenants)"]
async fn e2e_locks_do_not_block() {
    let holder = pg_client(PG_PORT, TENANTS_DB).await;
    holder
        .batch_execute("BEGIN; LOCK TABLE tenant_00002.orders IN ACCESS EXCLUSIVE MODE")
        .await
        .unwrap();

    let catalog = RwLock::new(Catalog::new());
    let mut introspector = connect(PG_PORT, TENANTS_DB, IntrospectOptions::default()).await;

    let start = Instant::now();
    sync(&mut introspector, &catalog).await;
    assert!(start.elapsed() < Duration::from_secs(10));
    assert_eq!(introspector.load_schemas(&catalog, &["tenant_00002".into()]).await.unwrap(), 1);

    let start = Instant::now();
    let error = introspector
        .relation_size("tenant_00002", "orders")
        .await
        .unwrap_err();
    assert!(start.elapsed() < Duration::from_secs(4), "{:?}", start.elapsed());
    assert!(error.to_string().contains("lock timeout"), "{error}");

    // A conexão continua utilizável depois do erro
    assert!(introspector.relation_size("tenant_00002", "customers").await.unwrap().is_some());
    holder.batch_execute("ROLLBACK").await.unwrap();
}

/// `statement_timeout` corta a varredura; o cancelamento interrompe no meio e
/// devolve `Cancelled`; nos dois casos a conexão segue utilizável.
#[tokio::test]
#[ignore = "perf: requer perf/catalog (seed.sh up + pg-tenants, de preferência tier M ou L)"]
async fn e2e_timeout_and_cancel() {
    let catalog = RwLock::new(Catalog::new());
    let mut introspector = connect(
        PG_PORT,
        TENANTS_DB,
        IntrospectOptions {
            statement_timeout: Duration::from_millis(1),
            ..Default::default()
        },
    )
    .await;
    let error = introspector.sync(&catalog, |_| {}).await.unwrap_err();
    assert!(error.to_string().contains("statement timeout"), "{error}");
    // A conexão segue viva: a próxima consulta roda — e, com 1 ms, às vezes
    // estoura o mesmo timeout, o que também prova que ela foi ao servidor
    match introspector.relation_size("public", "plans").await {
        Ok(size) => assert!(size.is_some()),
        Err(error) => assert!(error.to_string().contains("statement timeout"), "{error}"),
    }
    assert!(!introspector.is_closed());

    let mut introspector = connect(
        PG_PORT,
        TENANTS_DB,
        IntrospectOptions {
            force_bulk: true,
            ..Default::default()
        },
    )
    .await;
    let canceller = introspector.canceller();
    let start = Instant::now();
    let (result, _) = tokio::join!(introspector.sync(&catalog, |_| {}), async {
        tokio::time::sleep(Duration::from_millis(150)).await;
        canceller.cancel().await.unwrap();
    });
    let elapsed = start.elapsed();
    match result {
        Err(Error::Cancelled) => assert!(elapsed < Duration::from_millis(900), "{elapsed:?}"),
        // Tier pequeno: a varredura acabou antes do cancelamento chegar
        Ok(report) if report.total < Duration::from_millis(150) => {
            println!("sync terminou em {:?}, antes do cancelamento", report.total);
        }
        other => panic!("{other:?}"),
    }

    let admin = pg_client(PG_PORT, "postgres").await;
    let busy: i64 = admin
        .query_one(
            "SELECT count(*) FROM pg_stat_activity
             WHERE application_name = 'octapus_db catalog' AND state = 'active'",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(busy, 0);
    assert!(introspector.relation_size("public", "plans").await.unwrap().is_some());
}

/// Compatibilidade de versão: o mesmo diferencial num Postgres 14
/// (`seed.sh compat`, porta 55433).
#[tokio::test]
#[ignore = "perf: requer perf/catalog (seed.sh compat)"]
async fn e2e_compat_postgres_14() {
    let port = env_u64("OCTAPUS_PERF_COMPAT_PORT", 55_433) as u16;
    let catalog = RwLock::new(Catalog::new());
    let mut introspector = connect(port, TENANTS_DB, IntrospectOptions::default()).await;
    assert!(introspector.info().version.starts_with("PostgreSQL 14"));
    assert!(introspector.info().caps.shapes && introspector.info().caps.partitions);

    let report = sync(&mut introspector, &catalog).await;
    assert_eq!(report.strategy, Strategy::ShapeFirst);
    let catalog = catalog.read();
    assert_eq!(catalog.stats().schemas, 52);
    assert_eq!(dump(&catalog)["events"].len(), 301);
}

// ─────────────────────────────────────────────────────────────────────────────
// Desempenho
// ─────────────────────────────────────────────────────────────────────────────

/// Camadas no tier configurado: primeira sincronização, revalidação sem e com
/// mudança, carga prioritária, caminho em massa.
#[tokio::test]
#[ignore = "perf: requer perf/catalog (seed.sh up + pg-tenants)"]
async fn perf_layers() {
    let net = Net::from_env(PG_PORT).await;
    let report = Report::new("introspect", &net);
    let port = net.port(PG_PORT);
    let remote = net.label != "local";

    let start = Instant::now();
    let mut introspector = connect(port, TENANTS_DB, IntrospectOptions::default()).await;
    report.record("conectar (dedicada) + version()", Some(start.elapsed()), json!({}));

    let catalog = RwLock::new(Catalog::new());
    let first = sync(&mut introspector, &catalog).await;
    let stats = catalog.read().stats();
    report.record(
        "1ª sincronização: camada 0 (schemas visíveis)",
        Some(first.layer0),
        json!({ "schemas": first.schemas }),
    );
    report.record(
        "1ª sincronização: total (shape-first)",
        Some(first.total),
        json!({
            "shapes": first.shapes,
            "fetched": first.fetched,
            "shared": first.shared,
            "relations": stats.relations,
            "catalog_heap_mb": stats.approx_heap_bytes as f64 / 1e6,
        }),
    );
    if remote {
        assert!(first.layer0 <= Duration::from_millis(300), "camada 0 {:?}", first.layer0);
        assert!(first.total <= Duration::from_secs(3), "total {:?}", first.total);
    }

    let again = sync(&mut introspector, &catalog).await;
    report.record(
        "revalidação sem mudança",
        Some(again.total),
        json!({ "fetched": again.fetched, "shared": again.shared }),
    );
    assert_eq!(again.fetched + again.shared, 0);

    let admin = pg_client(PG_PORT, TENANTS_DB).await;
    admin
        .batch_execute("CREATE TABLE tenant_00007.perf_probe (id int)")
        .await
        .unwrap();
    let changed = sync(&mut introspector, &catalog).await;
    admin
        .batch_execute("DROP TABLE tenant_00007.perf_probe")
        .await
        .unwrap();
    report.record(
        "revalidação com 1 tenant alterado",
        Some(changed.total),
        json!({ "changed": changed.diff.changed, "fetched": changed.fetched }),
    );
    assert_eq!(changed.diff.changed, ["tenant_00007"]);
    assert_eq!(changed.fetched, 1);

    let fresh = RwLock::new(Catalog::new());
    let start = Instant::now();
    introspector
        .load_schemas(&fresh, &["tenant_04242".into()])
        .await
        .unwrap();
    let priority = start.elapsed();
    report.record("carga prioritária de 1 schema", Some(priority), json!({}));
    if remote {
        assert!(priority <= Duration::from_millis(600), "prioridade {priority:?}");
    }

    let start = Instant::now();
    let size = introspector.relation_size("tenant_00001", "orders").await.unwrap();
    report.record(
        "tamanho exato de 1 tabela (sob demanda)",
        Some(start.elapsed()),
        json!({ "bytes": size }),
    );


    let mut bulk = connect(
        port,
        TENANTS_DB,
        IntrospectOptions {
            force_bulk: true,
            ..Default::default()
        },
    )
    .await;
    let bulk_catalog = RwLock::new(Catalog::new());
    let bulk_report = sync(&mut bulk, &bulk_catalog).await;
    report.record(
        "1ª sincronização em massa (fallback)",
        Some(bulk_report.total),
        json!({ "layer0_ms": ms(bulk_report.layer0), "fetched": bulk_report.fetched }),
    );
    assert_eq!(bulk_catalog.read().stats().relations, stats.relations);
}
