use std::fs::{self, OpenOptions};
use std::future::Future;
use std::io::Write;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::models::{DatabaseType, Server};

use super::latency_proxy::LatencyProxy;

/// Portas do `perf/catalog/docker-compose.yml` — nunca as de um banco cadastrado.
pub const PG_PORT: u16 = 55432;
pub const PG_PASSWORD: &str = "perf";
pub const MONGO_PORT: u16 = 57017;

fn env_or(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_string())
}

pub fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

/// Rede da medição: direta (`local`) ou atrás do [`LatencyProxy`] (`remote`).
pub struct Net {
    pub label: String,
    proxy: Option<LatencyProxy>,
}

impl Net {
    /// Lê `OCTAPUS_PERF_NET`. No modo `remote` sobe um proxy para `upstream`.
    pub async fn from_env(upstream_port: u16) -> Self {
        if env_or("OCTAPUS_PERF_NET", "local") != "remote" {
            return Self {
                label: "local".into(),
                proxy: None,
            };
        }

        let rtt_ms = env_u64("OCTAPUS_PERF_RTT_MS", 80);
        let mbps = env_u64("OCTAPUS_PERF_MBPS", 100);
        let upstream: SocketAddr = ([127, 0, 0, 1], upstream_port).into();
        let proxy = LatencyProxy::start(
            upstream,
            Duration::from_millis(rtt_ms / 2),
            Some(mbps * 1_000_000 / 8),
        )
        .await;

        Self {
            label: format!("remote-{rtt_ms}ms-{mbps}mbps"),
            proxy: Some(proxy),
        }
    }

    /// Porta a usar no lugar de `direct` (a do proxy, quando houver).
    pub fn port(&self, direct: u16) -> u16 {
        self.proxy.as_ref().map_or(direct, |proxy| proxy.port)
    }
}

pub fn pg_server(port: u16) -> Server {
    Server {
        id: Some(-1),
        name: "perf".into(),
        db_type: DatabaseType::Postgres,
        host: "127.0.0.1".into(),
        port,
        username: "postgres".into(),
        password: PG_PASSWORD.into(),
        default_database: None,
        ssl_enabled: false,
        connection_uri: None,
        scope_databases: None,
        scope_schemas: None,
        created_at: 0,
    }
}

pub fn mongo_server(port: u16) -> Server {
    Server {
        id: Some(-1),
        name: "perf".into(),
        db_type: DatabaseType::Mongodb,
        host: "127.0.0.1".into(),
        port,
        username: String::new(),
        password: String::new(),
        default_database: None,
        ssl_enabled: false,
        connection_uri: None,
        scope_databases: None,
        scope_schemas: None,
        created_at: 0,
    }
}

pub fn pg_conn_string(port: u16, database: &str) -> String {
    format!("host=127.0.0.1 port={port} user=postgres password={PG_PASSWORD} dbname={database}")
}

/// Conexão direta (fora do app) com um database da fixture, para preparar e
/// observar cenários.
pub async fn pg_client(port: u16, database: &str) -> tokio_postgres::Client {
    let (client, connection) =
        tokio_postgres::connect(&pg_conn_string(port, database), tokio_postgres::NoTls)
            .await
            .expect("conexão direta com a fixture");
    tokio::spawn(async move {
        let _ = connection.await;
    });
    client
}

pub fn results_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../perf/catalog/results");
    fs::create_dir_all(&dir).expect("criar perf/catalog/results");
    dir
}

/// Grava cada medição como uma linha JSON em `results/<suite>.jsonl` e
/// também no stdout (visível com `--nocapture`).
pub struct Report {
    suite: &'static str,
    tier: String,
    net: String,
}

impl Report {
    pub fn new(suite: &'static str, net: &Net) -> Self {
        Self {
            suite,
            tier: env_or("OCTAPUS_PERF_TIER", "adhoc"),
            net: net.label.clone(),
        }
    }

    /// Para medições sem banco nem rede (o núcleo do catálogo).
    pub fn offline(suite: &'static str, tier: &str) -> Self {
        Self {
            suite,
            tier: tier.to_string(),
            net: "n/a".into(),
        }
    }

    pub fn record(&self, case: &str, elapsed: Option<Duration>, extra: Value) {
        let line = json!({
            "suite": self.suite,
            "tier": self.tier,
            "net": self.net,
            "case": case,
            "ms": elapsed.map(|d| (d.as_secs_f64() * 1000.0 * 10.0).round() / 10.0),
            "extra": extra,
            "at": chrono::Utc::now().to_rfc3339(),
        });
        println!("{line}");

        let path = results_dir().join(format!("{}.jsonl", self.suite));
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .expect("abrir jsonl de resultados");
        writeln!(file, "{line}").expect("gravar resultado");
    }

    /// Texto longo (planos do EXPLAIN) em `results/plans/<tier>-<net>-<nome>.txt`.
    pub fn save_text(&self, name: &str, text: &str) {
        let dir = results_dir().join("plans");
        fs::create_dir_all(&dir).expect("criar results/plans");
        fs::write(dir.join(format!("{}-{}-{name}.txt", self.tier, self.net)), text)
            .expect("gravar plano");
    }
}

pub async fn timed<T>(future: impl Future<Output = T>) -> (T, Duration) {
    let start = Instant::now();
    let value = future.await;
    (value, start.elapsed())
}

/// Mediana de `n` execuções sequenciais.
pub async fn median_of<F, Fut, T>(n: usize, mut run: F) -> Duration
where
    F: FnMut() -> Fut,
    Fut: Future<Output = T>,
{
    let mut samples = Vec::with_capacity(n);
    for _ in 0..n {
        let (_, elapsed) = timed(run()).await;
        samples.push(elapsed);
    }
    samples.sort();
    samples[n / 2]
}
