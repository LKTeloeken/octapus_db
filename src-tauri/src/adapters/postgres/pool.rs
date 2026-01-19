use std::time::Duration;

use deadpool_postgres::{Config, ManagerConfig, Pool, RecyclingMethod, Runtime};
use tokio_postgres::NoTls;

use crate::error::{Error, Result};
use crate::models::Server;

const POOL_MAX_SIZE: usize = 16;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const IDLE_TIMEOUT: Duration = Duration::from_secs(300);

pub fn create_pool(server: &Server, database: &str) -> Result<Pool> {
    let mut cfg = Config::new();

    cfg.host = Some(server.host.clone());
    cfg.port = Some(server.port);
    cfg.user = Some(server.username.clone());
    cfg.password = Some(server.password.clone());
    cfg.dbname = Some(database.to_string());
    cfg.connect_timeout = Some(CONNECT_TIMEOUT);

    cfg.manager = Some(ManagerConfig {
        recycling_method: RecyclingMethod::Fast,
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

    // TODO: Add SSL support based on server.ssl_enabled
    cfg.create_pool(Some(Runtime::Tokio1), NoTls)
        .map_err(|e| Error::Connection(e.to_string()))
}