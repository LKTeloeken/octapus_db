mod message_sink;
mod traits;
pub mod mongo;
pub mod postgres;
pub mod redisdb;
pub mod sqlite;

pub use message_sink::*;
pub use traits::*;

use std::sync::Arc;

use crate::catalog::CatalogSource;
use crate::error::{Error, Result};
use crate::models::{DatabaseType, Server};

/// Create an adapter for the given server and database
pub async fn create_adapter(
    server: &Server,
    database: &str,
) -> Result<Arc<dyn DatabaseAdapter>> {
    match server.db_type {
        DatabaseType::Postgres => {
            let adapter = postgres::PostgresAdapter::new(server, database)?;
            Ok(Arc::new(adapter))
        }
        DatabaseType::Mongodb => {
            let adapter = mongo::MongoAdapter::new(server, database).await?;
            Ok(Arc::new(adapter))
        }
        DatabaseType::Redis => {
            let adapter = redisdb::RedisAdapter::new(server, database)?;
            Ok(Arc::new(adapter))
        }
        DatabaseType::Mysql => {
            Err(Error::UnsupportedDatabase("MySQL support coming soon".into()))
        }
        DatabaseType::Sqlite => {
            let adapter = sqlite::SqliteAdapter::new(server, database)?;
            Ok(Arc::new(adapter))
        }
    }
}

/// Bancos cuja estrutura vem do catálogo de metadados. O Redis fica de fora:
/// as chaves não têm estrutura fixa, e a árvore dele já é por prefixo.
pub fn has_catalog(db_type: DatabaseType) -> bool {
    matches!(
        db_type,
        DatabaseType::Postgres | DatabaseType::Mongodb | DatabaseType::Sqlite
    )
}

/// Abre a fonte do catálogo de metadados (uma conexão dedicada) para um
/// database.
pub async fn create_catalog_source(server: &Server, database: &str) -> Result<Box<dyn CatalogSource>> {
    match server.db_type {
        DatabaseType::Postgres => {
            let options = postgres::introspect::IntrospectOptions {
                scope: crate::catalog::NameScope::parse(server.scope_schemas.as_deref()),
                ..Default::default()
            };
            let introspector = postgres::introspect::Introspector::connect(server, database, options).await?;
            Ok(Box::new(introspector))
        }
        DatabaseType::Mongodb => Ok(Box::new(mongo::introspect::MongoSource::connect(server, database).await?)),
        DatabaseType::Sqlite => Ok(Box::new(sqlite::introspect::SqliteSource::connect(server, database).await?)),
        other => Err(Error::UnsupportedDatabase(format!(
            "metadata catalog is not available for {other:?}"
        ))),
    }
}
