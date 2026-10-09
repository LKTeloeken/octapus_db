//! Baseline do Postgres: o custo de cada comando de estrutura de hoje e das
//! queries candidatas do catálogo novo, contra o database `tenants` da fixture.

use std::time::Duration;

use futures_util::{pin_mut, StreamExt};
use serde_json::json;
use tokio::time::{sleep, timeout, Instant};
use tokio_postgres::types::ToSql;
use tokio_postgres::{Client, NoTls};

use crate::adapters::postgres::PostgresAdapter;
use crate::adapters::DatabaseAdapter;

use super::fixture::{
    env_u64, median_of, pg_conn_string, pg_server, timed, Net, Report, PG_PORT,
};

const TENANTS_DB: &str = "tenants";

// ── Cópias das queries atuais de adapters/postgres/metadata.rs (baseline) ──

const LIST_SCHEMAS_SQL: &str = r#"
    SELECT n.nspname, COUNT(c.oid)::bigint as table_count
    FROM pg_namespace n
    LEFT JOIN pg_class c ON c.relnamespace = n.oid AND c.relkind IN ('r', 'v', 'm')
    WHERE n.nspname NOT IN ('pg_toast', 'pg_catalog', 'information_schema')
    GROUP BY n.nspname
    ORDER BY n.nspname
"#;

const LIST_TABLES_SQL: &str = r#"
    SELECT c.relname, n.nspname,
        CASE c.relkind WHEN 'r' THEN 'table' WHEN 'v' THEN 'view'
            WHEN 'm' THEN 'materialized_view' WHEN 'f' THEN 'foreign' END as table_type,
        c.reltuples::bigint as row_estimate,
        CASE WHEN c.relkind IN ('r', 'm') THEN pg_total_relation_size(c.oid) END AS size_bytes
    FROM pg_class c
    JOIN pg_namespace n ON n.oid = c.relnamespace
    WHERE n.nspname = $1 AND c.relkind IN ('r', 'v', 'm', 'f')
    ORDER BY c.relname
"#;

/// A listagem que o app fazia até a Fase 4 (`list_schemas_with_tables`), copiada
/// aqui quando saiu do adapter: a baseline continua reproduzível
pub(super) const LEGACY_STRUCTURE_SQL: &str = r#"
    SELECT n.nspname AS schema_name, c.relname AS table_name, c.relkind AS table_kind,
        CASE WHEN c.relkind IN ('r', 'm') THEN pg_total_relation_size(c.oid) END AS size_bytes
    FROM pg_namespace n
    LEFT JOIN pg_class c ON c.relnamespace = n.oid AND c.relkind IN ('r', 'v', 'm', 'f')
    WHERE n.nspname NOT IN ('pg_toast', 'pg_catalog', 'information_schema')
    ORDER BY n.nspname, c.relname
"#;

// ── Candidatas do catálogo novo ──

/// list_schemas_with_tables de hoje, sem o tamanho (que lê o disco e pega lock)
const STRUCTURE_NO_SIZE_SQL: &str = r#"
    SELECT n.nspname, c.relname, c.relkind
    FROM pg_namespace n
    LEFT JOIN pg_class c ON c.relnamespace = n.oid AND c.relkind IN ('r', 'v', 'm', 'f')
    WHERE n.nspname NOT IN ('pg_toast', 'pg_catalog', 'information_schema')
    ORDER BY n.nspname, c.relname
"#;

/// Idem, com o tamanho estimado pelo catálogo (relpages), sem tocar no disco
const STRUCTURE_RELPAGES_SQL: &str = r#"
    SELECT n.nspname, c.relname, c.relkind,
        CASE WHEN c.relkind IN ('r', 'm')
            THEN c.relpages::bigint * current_setting('block_size')::bigint END
    FROM pg_namespace n
    LEFT JOIN pg_class c ON c.relnamespace = n.oid AND c.relkind IN ('r', 'v', 'm', 'f')
    WHERE n.nspname NOT IN ('pg_toast', 'pg_catalog', 'information_schema')
    ORDER BY n.nspname, c.relname
"#;

/// Camada 0: só os schemas, já sem os temporários
const SCHEMAS_ONLY_SQL: &str = r#"
    SELECT n.oid, n.nspname
    FROM pg_namespace n
    WHERE n.nspname NOT IN ('pg_catalog', 'information_schema')
      AND n.nspname !~ '^pg_(toast|temp_|toast_temp_)'
"#;

/// Camada 1 em massa: sem join e sem ORDER BY (o agrupamento é local)
const TABLES_FLAT_SQL: &str = r#"
    SELECT c.relnamespace, c.relname, c.relkind, c.relispartition, c.relpages, c.reltuples
    FROM pg_class c
    WHERE c.relkind IN ('r', 'v', 'm', 'f', 'p')
"#;

/// Camada 1 por lote de schemas (pedido com prioridade)
const TABLES_BATCH_SQL: &str = r#"
    SELECT c.relnamespace, c.relname, c.relkind, c.relispartition, c.relpages, c.reltuples
    FROM pg_class c
    WHERE c.relkind IN ('r', 'v', 'm', 'f', 'p') AND c.relnamespace = ANY($1)
"#;

/// Fingerprint por schema: muda com CREATE/DROP/ALTER/RENAME (xmin novo na
/// linha do pg_class), não com VACUUM/ANALYZE (que gravam no lugar)
const FINGERPRINT_SQL: &str = r#"
    SELECT c.relnamespace, count(*), sum(hashtext(c.oid::text || ':' || c.xmin::text))
    FROM pg_class c
    WHERE c.relkind IN ('r', 'v', 'm', 'f', 'p')
    GROUP BY c.relnamespace
"#;

/// Shape-first: numa varredura só, o formato (conjunto de tabelas) e o
/// fingerprint de cada schema. Só as tabelas de um representante por formato
/// precisam atravessar a rede — em multi-tenant, poucos em vez de milhares.
const SHAPES_SQL: &str = r#"
    SELECT c.relnamespace,
        md5(string_agg(c.relname || ':' || c.relkind::text, ',' ORDER BY c.relname)) AS shape,
        count(*),
        sum(hashtext(c.oid::text || ':' || c.xmin::text)) AS fingerprint
    FROM pg_class c
    WHERE c.relkind IN ('r', 'v', 'm', 'f', 'p')
    GROUP BY c.relnamespace
"#;

/// Checagem pontual pelo índice (relname, relnamespace)
const PROBE_SQL: &str = r#"
    SELECT c.oid, c.relkind
    FROM pg_class c
    WHERE c.relname = $1
      AND c.relnamespace = (SELECT oid FROM pg_namespace WHERE nspname = $2)
"#;

/// "Em quais schemas existe `orders`?" — mesmo índice, sem o schema
const PROBE_ANY_SCHEMA_SQL: &str = r#"
    SELECT c.relnamespace, c.relkind FROM pg_class c WHERE c.relname = $1
"#;

async fn raw_client(port: u16, database: &str) -> Client {
    let (client, connection) = tokio_postgres::connect(&pg_conn_string(port, database), NoTls)
        .await
        .expect("conexão direta com a fixture");
    tokio::spawn(async move {
        let _ = connection.await;
    });
    client
}

struct StreamStats {
    first_row: Option<Duration>,
    total: Duration,
    rows: usize,
}

/// Consome a query em streaming medindo o tempo até a primeira linha.
async fn stream_rows(client: &Client, sql: &str, params: &[&(dyn ToSql + Sync)]) -> StreamStats {
    let start = Instant::now();
    let stream = client
        .query_raw(sql, params.iter().copied())
        .await
        .expect("query_raw");
    pin_mut!(stream);

    let mut first_row = None;
    let mut rows = 0;
    while let Some(row) = stream.next().await {
        row.expect("linha do stream");
        if first_row.is_none() {
            first_row = Some(start.elapsed());
        }
        rows += 1;
    }

    StreamStats {
        first_row,
        total: start.elapsed(),
        rows,
    }
}

/// `EXPLAIN (ANALYZE, BUFFERS)` com os parâmetros já embutidos como literais.
async fn explain(client: &Client, sql: &str) -> String {
    client
        .query(&format!("EXPLAIN (ANALYZE, BUFFERS) {sql}"), &[])
        .await
        .expect("explain")
        .iter()
        .map(|row| row.get::<_, String>(0))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Cancela (sempre pela porta direta) as queries ativas que contêm `needle`.
async fn cancel_backends(needle: &str) -> i64 {
    let client = raw_client(PG_PORT, "postgres").await;
    client
        .query_one(
            "SELECT count(pg_cancel_backend(pid)) FROM pg_stat_activity
             WHERE pid <> pg_backend_pid() AND state = 'active'
               AND query LIKE '%' || $1 || '%'",
            &[&needle],
        )
        .await
        .map(|row| row.get(0))
        .unwrap_or(0)
}

fn ms(duration: Duration) -> f64 {
    (duration.as_secs_f64() * 1000.0 * 10.0).round() / 10.0
}

#[tokio::test]
#[ignore = "perf: requer perf/catalog (seed.sh up + pg-tenants)"]
async fn perf_pg_server_level() {
    let net = Net::from_env(PG_PORT).await;
    let report = Report::new("pg", &net);
    let port = net.port(PG_PORT);

    let (adapter, elapsed) = timed(async {
        let adapter = PostgresAdapter::new(&pg_server(port), "postgres").unwrap();
        adapter.test_connection().await.unwrap();
        adapter
    })
    .await;
    report.record("connect+ping", Some(elapsed), json!({}));

    let (databases, elapsed) = timed(adapter.list_databases()).await;
    report.record(
        "list_databases (atual, pg_database_size de todos)",
        Some(elapsed),
        json!({ "databases": databases.unwrap().len() }),
    );

    let client = raw_client(port, "postgres").await;
    let (rows, elapsed) = timed(client.query(
        "SELECT datname FROM pg_database WHERE NOT datistemplate ORDER BY 1",
        &[],
    ))
    .await;
    report.record(
        "list_databases (só nomes)",
        Some(elapsed),
        json!({ "databases": rows.unwrap().len() }),
    );

    let (_, elapsed) = timed(client.query_one("SELECT pg_database_size('tenants')", &[])).await;
    report.record("pg_database_size(tenants)", Some(elapsed), json!({}));
}

#[tokio::test]
#[ignore = "perf: requer perf/catalog (seed.sh up + pg-tenants)"]
async fn perf_pg_tree_current() {
    let net = Net::from_env(PG_PORT).await;
    let report = Report::new("pg", &net);
    let port = net.port(PG_PORT);

    let adapter = PostgresAdapter::new(&pg_server(port), TENANTS_DB).unwrap();
    adapter.test_connection().await.unwrap();

    let (schemas, elapsed) = timed(adapter.list_schemas()).await;
    let schemas = schemas.unwrap();
    let tenants: Vec<&str> = schemas
        .iter()
        .map(|schema| schema.name.as_str())
        .filter(|name| name.starts_with("tenant_"))
        .collect();
    report.record(
        "list_schemas (atual)",
        Some(elapsed),
        json!({ "schemas": schemas.len() }),
    );

    // Expandir um schema: hoje cada um custa uma varredura do pg_class
    let samples = [
        tenants[0],
        tenants[tenants.len() / 2],
        tenants[tenants.len() - 1],
        "public",
        "events",
    ];
    for schema in samples {
        let (tables, elapsed) = timed(adapter.list_tables(schema)).await;
        report.record(
            "list_tables (atual, 1 schema)",
            Some(elapsed),
            json!({ "schema": schema, "tables": tables.unwrap().len() }),
        );
    }

    // No tier L a listagem atual derruba o backend (OOM); depois de medir uma
    // vez, OCTAPUS_PERF_SKIP_FULL=1 evita repetir o estrago.
    if env_u64("OCTAPUS_PERF_SKIP_FULL", 0) == 1 {
        return;
    }

    let limit = Duration::from_secs(env_u64("OCTAPUS_PERF_FULL_TIMEOUT_S", 900));
    let client = raw_client(port, TENANTS_DB).await;
    let (outcome, elapsed) = timed(timeout(limit, client.query(LEGACY_STRUCTURE_SQL, &[]))).await;
    match outcome {
        Ok(Ok(rows)) => {
            let schemas: std::collections::BTreeSet<String> =
                rows.iter().map(|row| row.get::<_, String>(0)).collect();
            report.record(
                "list_schemas_with_tables (atual)",
                Some(elapsed),
                json!({ "schemas": schemas.len(), "rows": rows.len() }),
            );
        }
        Ok(Err(error)) => report.record(
            "list_schemas_with_tables (atual)",
            Some(elapsed),
            json!({ "error": crate::error::Error::from(error).to_string() }),
        ),
        Err(_) => {
            let cancelled = cancel_backends("pg_total_relation_size").await;
            report.record(
                "list_schemas_with_tables (atual)",
                Some(elapsed),
                json!({ "timeout_s": limit.as_secs(), "cancelled_backends": cancelled }),
            );
        }
    }
}

#[tokio::test]
#[ignore = "perf: requer perf/catalog (seed.sh up + pg-tenants)"]
async fn perf_pg_catalog_candidates() {
    let net = Net::from_env(PG_PORT).await;
    let report = Report::new("pg", &net);
    let client = raw_client(net.port(PG_PORT), TENANTS_DB).await;

    for (case, sql) in [
        ("camada0: schemas", SCHEMAS_ONLY_SQL),
        ("camada1: tabelas em massa (sem join/ordem)", TABLES_FLAT_SQL),
        ("estrutura atual sem tamanho", STRUCTURE_NO_SIZE_SQL),
        ("estrutura atual com tamanho por relpages", STRUCTURE_RELPAGES_SQL),
        ("fingerprint global", FINGERPRINT_SQL),
    ] {
        let stats = stream_rows(&client, sql, &[]).await;
        report.record(
            case,
            Some(stats.total),
            json!({ "rows": stats.rows, "first_row_ms": stats.first_row.map(ms) }),
        );
    }

    // Bytes de nome que uma transferência compacta carregaria (sem JSON)
    let name_bytes: i64 = client
        .query_one(
            "SELECT sum(octet_length(relname))::bigint FROM pg_class
             WHERE relkind IN ('r', 'v', 'm', 'f', 'p')",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    report.record("bytes de nomes de tabela", None, json!({ "bytes": name_bytes }));

    // Shape-first: formatos + fingerprints, depois as tabelas de um representante
    // por formato
    let start = Instant::now();
    let shape_rows = client.query(SHAPES_SQL, &[]).await.unwrap();
    let shapes_elapsed = start.elapsed();
    let mut representatives: std::collections::HashMap<String, u32> = Default::default();
    for row in &shape_rows {
        representatives.entry(row.get(1)).or_insert(row.get(0));
    }
    let representative_oids: Vec<u32> = representatives.values().copied().collect();
    let tables = stream_rows(&client, TABLES_BATCH_SQL, &[&representative_oids]).await;
    report.record(
        "shape-first: formatos + fingerprint (1 varredura)",
        Some(shapes_elapsed),
        json!({ "schemas": shape_rows.len(), "shapes": representatives.len() }),
    );
    report.record(
        "shape-first: tabelas dos representantes",
        Some(tables.total),
        json!({ "rows": tables.rows }),
    );
    report.record(
        "shape-first: total",
        Some(start.elapsed()),
        json!({ "shapes": representatives.len() }),
    );

    // Lote de 200 schemas: a fila de prioridade junta pedidos numa varredura só
    let tenant_oids: Vec<u32> = client
        .query(
            "SELECT oid FROM pg_namespace WHERE nspname LIKE 'tenant_%' ORDER BY nspname LIMIT 200",
            &[],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| row.get(0))
        .collect();
    let stats = stream_rows(&client, TABLES_BATCH_SQL, &[&tenant_oids]).await;
    report.record(
        "camada1: lote de 200 schemas",
        Some(stats.total),
        json!({ "schemas": tenant_oids.len(), "rows": stats.rows }),
    );

    let elapsed = median_of(5, || async {
        client.query(PROBE_SQL, &[&"orders", &"tenant_00001"]).await
    })
    .await;
    report.record("probe por nome (schema.tabela)", Some(elapsed), json!({ "median_of": 5 }));

    let (rows, elapsed) = timed(client.query(PROBE_ANY_SCHEMA_SQL, &[&"orders"])).await;
    report.record(
        "probe por nome (tabela em qualquer schema)",
        Some(elapsed),
        json!({ "rows": rows.unwrap().len() }),
    );

    // Planos: provam seq scan vs index scan
    let oids_literal = format!(
        "ARRAY[{}]::oid[]",
        tenant_oids.iter().map(u32::to_string).collect::<Vec<_>>().join(",")
    );
    for (name, sql) in [
        ("list_tables-1-schema", LIST_TABLES_SQL.replace("$1", "'tenant_00001'")),
        ("tables-batch-200", TABLES_BATCH_SQL.replace("$1", &oids_literal)),
        (
            "probe",
            PROBE_SQL.replace("$1", "'orders'").replace("$2", "'tenant_00001'"),
        ),
        ("probe-any-schema", PROBE_ANY_SCHEMA_SQL.replace("$1", "'orders'")),
        ("fingerprint", FINGERPRINT_SQL.to_string()),
        ("shapes", SHAPES_SQL.to_string()),
    ] {
        report.save_text(name, &explain(&client, &sql).await);
    }
}

/// Memória total do backend desta sessão (todos os memory contexts).
async fn backend_memory(client: &Client) -> i64 {
    client
        .query_one(
            "SELECT sum(total_bytes)::bigint FROM pg_backend_memory_contexts",
            &[],
        )
        .await
        .unwrap()
        .get(0)
}

/// Hipótese H7: `pg_total_relation_size` abre cada relação, e o backend guarda
/// uma entrada de relcache por relação aberta até a conexão fechar. No tier L
/// isso levou o backend ao OOM killer — aqui medimos o custo por tabela num
/// recorte de `OCTAPUS_PERF_RELCACHE_SCHEMAS` schemas e extrapolamos.
#[tokio::test]
#[ignore = "perf: requer perf/catalog (seed.sh up + pg-tenants)"]
async fn perf_pg_relcache_bloat() {
    let net = Net::from_env(PG_PORT).await;
    let report = Report::new("pg", &net);
    let schemas = env_u64("OCTAPUS_PERF_RELCACHE_SCHEMAS", 300) as i64;
    let schema_filter = "SELECT oid FROM pg_namespace WHERE nspname LIKE 'tenant_%' \
                         ORDER BY nspname LIMIT $1";

    // Leitura pura do catálogo (candidata): não deve inchar o backend
    let client = raw_client(PG_PORT, TENANTS_DB).await;
    let before = backend_memory(&client).await;
    let (rows, elapsed) = timed(client.query(
        &format!(
            "SELECT c.relname FROM pg_class c
             WHERE c.relkind IN ('r', 'm') AND c.relnamespace IN ({schema_filter})"
        ),
        &[&schemas],
    ))
    .await;
    let tables = rows.unwrap().len() as i64;
    let delta = backend_memory(&client).await - before;
    report.record(
        "relcache: leitura do catálogo (candidata)",
        Some(elapsed),
        json!({ "tables": tables, "backend_delta_mb": delta / 1_000_000 }),
    );

    // O que a listagem atual faz por tabela, numa sessão nova
    let client = raw_client(PG_PORT, TENANTS_DB).await;
    let before = backend_memory(&client).await;
    let (outcome, elapsed) = timed(client.query_one(
        &format!(
            "SELECT count(pg_total_relation_size(c.oid)) FROM pg_class c
             WHERE c.relkind IN ('r', 'm') AND c.relnamespace IN ({schema_filter})"
        ),
        &[&schemas],
    ))
    .await;
    outcome.unwrap();
    let delta = backend_memory(&client).await - before;
    let per_table = delta / tables.max(1);
    report.record(
        "relcache: pg_total_relation_size (atual)",
        Some(elapsed),
        json!({
            "tables": tables,
            "backend_delta_mb": delta / 1_000_000,
            "bytes_por_tabela": per_table,
            "projecao_750k_tabelas_gb": (per_table * 750_000) as f64 / 1e9,
        }),
    );
}

/// Hipótese H2: uma migração segurando ACCESS EXCLUSIVE em uma tabela de um
/// tenant trava a listagem atual (pg_total_relation_size pega AccessShareLock
/// em cada relação), mas não a leitura pura do catálogo.
#[tokio::test]
#[ignore = "perf: requer perf/catalog (seed.sh up + pg-tenants)"]
async fn perf_pg_lock_wait() {
    let net = Net::from_env(PG_PORT).await;
    let report = Report::new("pg", &net);
    let port = net.port(PG_PORT);
    let wait = Duration::from_secs(env_u64("OCTAPUS_PERF_LOCK_WAIT_S", 20));

    let holder = raw_client(PG_PORT, TENANTS_DB).await;
    holder
        .batch_execute("BEGIN; LOCK TABLE tenant_00002.orders IN ACCESS EXCLUSIVE MODE")
        .await
        .unwrap();

    let watcher = raw_client(PG_PORT, "postgres").await;
    let observe = || async {
        sleep(wait / 2).await;
        watcher
            .query(
                "SELECT coalesce(wait_event_type, ''), coalesce(wait_event, '')
                 FROM pg_stat_activity
                 WHERE pid <> pg_backend_pid() AND state = 'active'
                   AND query LIKE '%pg_total_relation_size%'",
                &[],
            )
            .await
            .unwrap()
            .iter()
            .map(|row| format!("{}:{}", row.get::<_, String>(0), row.get::<_, String>(1)))
            .collect::<Vec<_>>()
    };

    let adapter = PostgresAdapter::new(&pg_server(port), TENANTS_DB).unwrap();
    adapter.test_connection().await.unwrap();
    let legacy = raw_client(port, TENANTS_DB).await;

    let ((outcome, waits), elapsed) = timed(async {
        tokio::join!(timeout(wait, legacy.query(LEGACY_STRUCTURE_SQL, &[])), observe())
    })
    .await;
    let blocked = outcome.is_err();
    let cancelled = cancel_backends("pg_total_relation_size").await;
    report.record(
        "lock: list_schemas_with_tables (atual)",
        Some(elapsed),
        json!({ "blocked": blocked, "wait_events": waits, "cancelled_backends": cancelled }),
    );

    let ((outcome, waits), elapsed) =
        timed(async { tokio::join!(timeout(wait, adapter.list_tables("tenant_00002")), observe()) })
            .await;
    let blocked = outcome.is_err();
    let cancelled = cancel_backends("pg_total_relation_size").await;
    report.record(
        "lock: list_tables do schema travado (atual)",
        Some(elapsed),
        json!({ "blocked": blocked, "wait_events": waits, "cancelled_backends": cancelled }),
    );

    let client = raw_client(port, TENANTS_DB).await;
    let (outcome, elapsed) = timed(timeout(wait, stream_rows(&client, TABLES_FLAT_SQL, &[]))).await;
    report.record(
        "lock: camada1 em massa (candidata)",
        Some(elapsed),
        json!({ "blocked": outcome.is_err(), "rows": outcome.map(|s| s.rows).ok() }),
    );

    holder.batch_execute("ROLLBACK").await.unwrap();
}

/// Hipótese H6: idas e voltas fixas por comando. Cada medição é a mediana de 7,
/// e `rtt` (um simple_query) é a unidade — rode com OCTAPUS_PERF_NET=remote.
#[tokio::test]
#[ignore = "perf: requer perf/catalog (seed.sh up + pg-tenants)"]
async fn perf_pg_command_overhead() {
    let net = Net::from_env(PG_PORT).await;
    let report = Report::new("pg", &net);
    let port = net.port(PG_PORT);
    const N: usize = 7;

    let raw = raw_client(port, TENANTS_DB).await;
    raw.simple_query("SELECT 1").await.unwrap();

    let rtt = median_of(N, || raw.simple_query("SELECT 1")).await;
    let prepared_query = median_of(N, || raw.query("SELECT 1", &[])).await;
    let raw_list_schemas = median_of(N, || raw.query(LIST_SCHEMAS_SQL, &[])).await;

    let adapter = PostgresAdapter::new(&pg_server(port), TENANTS_DB).unwrap();
    adapter.test_connection().await.unwrap();

    let ping = median_of(N, || adapter.test_connection()).await;
    let list_schemas = median_of(N, || adapter.list_schemas()).await;
    let connect_adapter_pattern = median_of(N, || async {
        adapter.test_connection().await.unwrap();
        adapter.list_schemas().await
    })
    .await;

    let unit = rtt.as_secs_f64().max(f64::EPSILON);
    for (case, elapsed) in [
        ("rtt (simple_query SELECT 1)", rtt),
        ("query com &str (prepare + execute)", prepared_query),
        ("list_schemas cru (conexão segura, sem pool)", raw_list_schemas),
        ("test_connection do adapter (ping)", ping),
        ("list_schemas pelo adapter (pool.get + query)", list_schemas),
        ("padrão connect_adapter (ping + list_schemas)", connect_adapter_pattern),
    ] {
        report.record(
            &format!("overhead: {case}"),
            Some(elapsed),
            json!({ "em_rtts": (elapsed.as_secs_f64() / unit * 10.0).round() / 10.0 }),
        );
    }
}
