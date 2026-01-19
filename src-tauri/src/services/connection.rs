use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::RwLock;

use crate::adapters::{create_adapter, DatabaseAdapter, PoolStats, PooledAdapter};
use crate::error::{Error, Result};
use crate::models::{ConnectionId, Server};

pub struct ConnectionService {
    adapters: RwLock<HashMap<ConnectionId, Arc<dyn DatabaseAdapter>>>,
}

impl ConnectionService {
    pub fn new() -> Self {
        Self {
            adapters: RwLock::new(HashMap::new()),
        }
    }

    /// Get or create an adapter for the given connection
    pub fn get_or_connect(
        &self,
        server: &Server,
        database: &str,
    ) -> Result<Arc<dyn DatabaseAdapter>> {
        let server_id = server.id.ok_or(Error::InvalidState("Server has no ID".into()))?;
        let conn_id = ConnectionId::new(server_id, database);

        // Fast path: adapter exists
        {
            let adapters = self.adapters.read();
            if let Some(adapter) = adapters.get(&conn_id) {
                return Ok(Arc::clone(adapter));
            }
        }

        // Slow path: create adapter
        let adapter = create_adapter(server, database)?;

        let mut adapters = self.adapters.write();
        adapters.insert(conn_id, Arc::clone(&adapter));

        Ok(adapter)
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

        adapters.get(&conn_id).and_then(|adapter| {
            // Try to downcast to PooledAdapter
            // This is a bit awkward; in real code you might use a different approach
            None // TODO: implement proper downcasting
        })
    }
}