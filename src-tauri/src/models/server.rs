use std::fmt;

use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

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

/// Marcador que ocupa o lugar de um segredo em qualquer `{:?}`.
struct Redacted(bool);

impl fmt::Debug for Redacted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(if self.0 { "<redigido>" } else { "<vazio>" })
    }
}

/// **`Debug` é escrito à mão de propósito.** O `derive` imprimiria a senha e a
/// URI (que carrega credencial) em qualquer `dbg!`, mensagem de panic ou log
/// futuro. **`Drop` também:** o Rust libera a `String` sem apagar os bytes, e a
/// senha ficaria legível no heap até a memória ser reaproveitada — o `zeroize`
/// sobrescreve na saída. Cópias dentro dos pools (deadpool/mongodb/redis)
/// continuam fora do nosso alcance.
#[derive(Clone, Serialize, Deserialize)]
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
    /// Serializada **redigida** (ver `repositories::servers`).
    pub connection_uri: Option<String>,
    pub created_at: i64,
}

impl fmt::Debug for Server {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Server")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("db_type", &self.db_type)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("username", &self.username)
            .field("password", &Redacted(!self.password.is_empty()))
            .field("default_database", &self.default_database)
            .field("ssl_enabled", &self.ssl_enabled)
            .field("connection_uri", &Redacted(self.connection_uri.is_some()))
            .field("created_at", &self.created_at)
            .finish()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.password.zeroize();
        self.connection_uri.zeroize();
    }
}

/// For creating/updating servers (receives password)
#[derive(Deserialize)]
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

impl fmt::Debug for ServerInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServerInput")
            .field("name", &self.name)
            .field("db_type", &self.db_type)
            .field("host", &self.host)
            .field("port", &self.port)
            .field("username", &self.username)
            .field("password", &Redacted(!self.password.is_empty()))
            .field("default_database", &self.default_database)
            .field("ssl_enabled", &self.ssl_enabled)
            .field("connection_uri", &Redacted(self.connection_uri.is_some()))
            .finish()
    }
}

impl Drop for ServerInput {
    fn drop(&mut self) {
        self.password.zeroize();
        self.connection_uri.zeroize();
    }
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
