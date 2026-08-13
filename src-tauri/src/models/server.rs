use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseType {
    Postgres,
    Mysql,
    Sqlite,
    Mongodb,
    Redis,
}

impl DatabaseType {
    /// Default port per database type, for prefilling the server form.
    #[allow(dead_code)]
    pub fn default_port(&self) -> u16 {
        match self {
            Self::Postgres => 5432,
            Self::Mysql => 3306,
            Self::Sqlite => 0,
            Self::Mongodb => 27017,
            Self::Redis => 6379,
        }
    }

    /// Database to connect to when the user hasn't picked one yet
    /// (e.g. for listing the server's databases).
    pub fn default_database_name(&self) -> &'static str {
        match self {
            Self::Postgres => "postgres",
            Self::Mysql => "mysql",
            Self::Sqlite => "main",
            Self::Mongodb => "admin",
            Self::Redis => "0",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Server {
    pub id: Option<i64>,
    pub name: String,
    pub db_type: DatabaseType,
    pub host: String,
    pub port: u16,
    pub username: String,
    #[serde(skip_serializing)] // Never send password to frontend
    pub password: String,
    pub default_database: Option<String>,
    pub ssl_enabled: bool,
    /// Full connection URI (e.g. mongodb+srv://... or rediss://...); when
    /// present it takes precedence over host/port/username for connecting.
    pub connection_uri: Option<String>,
    pub created_at: i64,
}

/// For creating/updating servers (receives password)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerInput {
    pub name: String,
    pub db_type: DatabaseType,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub default_database: Option<String>,
    pub ssl_enabled: Option<bool>,
    pub connection_uri: Option<String>,
}

/// Connection identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ConnectionId {
    pub server_id: i64,
    pub database: String,
}

impl ConnectionId {
    pub fn new(server_id: i64, database: impl Into<String>) -> Self {
        Self {
            server_id,
            database: database.into(),
        }
    }
}