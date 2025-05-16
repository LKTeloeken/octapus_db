// filepath: /Users/lucasteloeken/Documents/projects/octapus_db/src-tauri/src/db_manager/connection_pool.rs
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use rusqlite::Connection;
use r2d2::{Pool, PooledConnection, r2d2::Config};
use r2d2_sqlite::SqliteConnectionManager;

#[derive(Clone)]
pub struct ConnectionPool {
    pool: Arc<Mutex<Pool<SqliteConnectionManager>>>,
}

impl ConnectionPool {
    pub fn new(db_path: &str) -> Result<Self, String> {
        let manager = SqliteConnectionManager::file(db_path);
        let pool = Pool::builder()
            .max_size(15)
            .build(manager)
            .map_err(|e| e.to_string())?;
        
        Ok(ConnectionPool {
            pool: Arc::new(Mutex::new(pool)),
        })
    }

    pub fn get_connection(&self) -> Result<PooledConnection<SqliteConnectionManager>, String> {
        let pool = self.pool.lock().map_err(|e| e.to_string())?;
        pool.get().map_err(|e| e.to_string())
    }
}