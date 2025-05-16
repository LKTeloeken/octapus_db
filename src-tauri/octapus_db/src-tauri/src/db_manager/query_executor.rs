// filepath: /Users/lucasteloeken/Documents/projects/octapus_db/src-tauri/src/db_manager/query_executor.rs
use rusqlite::Connection;
use std::sync::Arc;
use crate::db_manager::connection_pool::ConnectionPool;
use crate::models::connection::DatabaseConnection;

pub struct QueryExecutor {
    pool: Arc<ConnectionPool>,
}

impl QueryExecutor {
    pub fn new(pool: Arc<ConnectionPool>) -> Self {
        QueryExecutor { pool }
    }

    pub fn execute_query(&self, db_connection: &DatabaseConnection, query: &str) -> Result<Vec<Vec<String>>, String> {
        let conn = self.pool.get_connection(db_connection).map_err(|e| e.to_string())?;
        
        let mut stmt = conn.prepare(query).map_err(|e| e.to_string())?;
        let rows = stmt.query_map([], |row| {
            let mut result_row = Vec::new();
            for i in 0..row.len() {
                result_row.push(row.get::<_, String>(i)?);
            }
            Ok(result_row)
        }).map_err(|e| e.to_string())?;

        let mut results = Vec::new();
        for row in rows {
            results.push(row.map_err(|e| e.to_string())?);
        }

        Ok(results)
    }
}