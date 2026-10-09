use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::{Mutex, RwLock};

use crate::adapters::{create_adapter, DatabaseAdapter, PoolStats};
use crate::error::{Error, Result};
use crate::models::{ConnectionId, Server};

/// Um adapter aberto e quando foi usado pela última vez.
struct Cached {
    adapter: Arc<dyn DatabaseAdapter>,
    last_used: Mutex<Instant>,
}

impl Cached {
    fn new(adapter: Arc<dyn DatabaseAdapter>) -> Arc<Self> {
        Arc::new(Self {
            adapter,
            last_used: Mutex::new(Instant::now()),
        })
    }

    /// Marca o uso e devolve há quanto tempo estava parado.
    fn touch(&self) -> Duration {
        let mut last_used = self.last_used.lock();
        let idle = last_used.elapsed();
        *last_used = Instant::now();
        idle
    }
}

pub struct ConnectionService {
    adapters: RwLock<HashMap<ConnectionId, Arc<Cached>>>,
}

impl ConnectionService {
    pub fn new() -> Self {
        Self {
            adapters: RwLock::new(HashMap::new()),
        }
    }

    /// Return an already-open adapter, or `None` if the connection isn't
    /// established yet, together with how long it sat unused (callers decide
    /// whether a liveness ping is worth it). Never creates a pool, so it never
    /// needs the password.
    pub fn get_cached(
        &self,
        server_id: i64,
        database: &str,
    ) -> Option<(Arc<dyn DatabaseAdapter>, Duration)> {
        let conn_id = ConnectionId::new(server_id, database);
        let adapters = self.adapters.read();
        let cached = adapters.get(&conn_id)?;
        let idle = cached.touch();
        Some((Arc::clone(&cached.adapter), idle))
    }

    /// O adapter em cache, se está vivo: usado há pouco volta direto; parado
    /// há mais que `ping_after` leva um ping antes — e sai do cache se falhar
    /// (quem chama reconecta). O ping incondicional custava três idas e voltas
    /// em todo comando (perf/catalog/BASELINE.md, H6).
    pub async fn get_live(
        &self,
        server_id: i64,
        database: &str,
        ping_after: Duration,
    ) -> Option<Arc<dyn DatabaseAdapter>> {
        let (adapter, idle) = self.get_cached(server_id, database)?;
        if idle < ping_after || adapter.test_connection().await.is_ok() {
            return Some(adapter);
        }
        self.disconnect(server_id, database);
        None
    }

    /// Get or create an adapter for the given connection
    pub async fn get_or_connect(
        &self,
        server: &Server,
        database: &str,
    ) -> Result<Arc<dyn DatabaseAdapter>> {
        let server_id = server.id.ok_or(Error::InvalidState("Server has no ID".into()))?;
        let conn_id = ConnectionId::new(server_id, database);

        // Fast path: adapter exists (lock is not held across the await below)
        {
            let adapters = self.adapters.read();
            if let Some(cached) = adapters.get(&conn_id) {
                cached.touch();
                return Ok(Arc::clone(&cached.adapter));
            }
        }

        // Slow path: create adapter. Concurrent creations may race; the
        // first inserted adapter wins and duplicates are dropped.
        let adapter = create_adapter(server, database).await?;

        let mut adapters = self.adapters.write();
        Ok(Arc::clone(
            &adapters.entry(conn_id).or_insert_with(|| Cached::new(adapter)).adapter,
        ))
    }

    /// Fecha os adapters sem uso há mais que `max_idle` (o pool fecha as
    /// conexões quando o último comando em andamento solta o adapter).
    pub fn evict_idle(&self, max_idle: Duration) -> usize {
        let mut adapters = self.adapters.write();
        let before = adapters.len();
        adapters.retain(|_, cached| cached.last_used.lock().elapsed() <= max_idle);
        before - adapters.len()
    }

    /// Disconnect from a specific database
    pub fn disconnect(&self, server_id: i64, database: &str) {
        let conn_id = ConnectionId::new(server_id, database);
        let mut adapters = self.adapters.write();
        adapters.remove(&conn_id);
    }

    /// Disconnect all databases for a server
    pub fn disconnect_server(&self, server_id: i64) {
        let mut adapters = self.adapters.write();
        adapters.retain(|k, _| k.server_id != server_id);
    }

    /// Get pool stats for a connection
    pub fn pool_stats(&self, server_id: i64, database: &str) -> Option<PoolStats> {
        let conn_id = ConnectionId::new(server_id, database);
        let adapters = self.adapters.read();
        adapters.get(&conn_id).and_then(|cached| cached.adapter.pool_stats())
    }
}