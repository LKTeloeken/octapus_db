mod traits;
pub mod postgres;

pub use traits::*;

use std::sync::Arc;

use crate::error::{Error, Result};
use crate::models::{DatabaseType, Server};

/// Create an adapter for the given server and database
pub fn create_adapter(
    server: &Server,
    database: &str,
) -> Result<Arc<dyn DatabaseAdapter>> {
    match server.db_type {
        DatabaseType::Postgres => {
            let adapter = postgres::PostgresAdapter::new(server, database)?;
            Ok(Arc::new(adapter))
        }
        DatabaseType::Mysql => {
            Err(Error::UnsupportedDatabase("MySQL support coming soon".into()))
        }
        DatabaseType::Sqlite => {
            Err(Error::UnsupportedDatabase("SQLite support coming soon".into()))
        }
    }
}