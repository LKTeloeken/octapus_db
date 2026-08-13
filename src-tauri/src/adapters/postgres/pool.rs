use std::time::Duration;

use deadpool_postgres::{Config, ManagerConfig, Pool, RecyclingMethod, Runtime};
use postgres_native_tls::MakeTlsConnector;
use tokio_postgres::NoTls;

use crate::error::{Error, Result};
use crate::models::Server;

const POOL_MAX_SIZE: usize = 16;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

pub fn create_pool(server: &Server, database: &str) -> Result<Pool> {
    let mut cfg = Config::new();

    if let Some(uri) = server.connection_uri.as_deref() {
        // The URI provides host/port/user/password; the selected database
        // still takes precedence so one server can open several databases.
        cfg.url = Some(uri.to_string());
    } else {
        cfg.host = Some(server.host.clone());
        cfg.port = Some(server.port);
        cfg.user = Some(server.username.clone());
        cfg.password = Some(server.password.clone());
    }
    cfg.dbname = Some(database.to_string());
    cfg.connect_timeout = Some(CONNECT_TIMEOUT);

    // Keepalive TCP: evita que firewalls/NAT derrubem conexões ociosas em
    // silêncio enquanto o app fica aberto sem uso.
    cfg.keepalives = Some(true);
    cfg.keepalives_idle = Some(Duration::from_secs(60));

    cfg.manager = Some(ManagerConfig {
        // Verified: valida a conexão a cada checkout do pool, descartando e
        // recriando as que morreram durante a ociosidade (Fast reusaria uma
        // conexão morta e o comando falharia na mão do usuário).
        recycling_method: RecyclingMethod::Verified,
    });

    cfg.pool = Some(deadpool_postgres::PoolConfig {
        max_size: POOL_MAX_SIZE,
        timeouts: deadpool_postgres::Timeouts {
            wait: Some(Duration::from_secs(30)),
            create: Some(CONNECT_TIMEOUT),
            recycle: Some(Duration::from_secs(5)),
        },
        ..Default::default()
    });

    if server.ssl_enabled {
        // sslmode=require semantics: encrypt the connection without
        // verifying the certificate (common for self-signed DB servers)
        let connector = native_tls::TlsConnector::builder()
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true)
            .build()
            .map_err(|e| Error::Connection(format!("TLS setup failed: {e}")))?;

        cfg.create_pool(Some(Runtime::Tokio1), MakeTlsConnector::new(connector))
            .map_err(|e| Error::Connection(e.to_string()))
    } else {
        cfg.create_pool(Some(Runtime::Tokio1), NoTls)
            .map_err(|e| Error::Connection(e.to_string()))
    }
}
