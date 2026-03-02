use std::fmt;

#[derive(Debug)]
#[allow(dead_code)]
pub enum Error {
    // Storage errors (local SQLite)
    Storage(String),

    // Connection errors
    Connection(String),
    PoolExhausted,
    ConnectionTimeout,

    // Query errors
    Query(String),
    InvalidQuery(String),

    // Resource errors
    NotFound(String),
    AlreadyExists(String),

    // State errors
    InvalidState(String),

    // Adapter errors
    UnsupportedDatabase(String),
    UnsupportedType(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(msg) => write!(f, "Storage error: {msg}"),
            Self::Connection(msg) => write!(f, "Connection error: {msg}"),
            Self::PoolExhausted => write!(f, "Connection pool exhausted"),
            Self::ConnectionTimeout => write!(f, "Connection timed out"),
            Self::Query(msg) => write!(f, "Query error: {msg}"),
            Self::InvalidQuery(msg) => write!(f, "Invalid query: {msg}"),
            Self::NotFound(msg) => write!(f, "Not found: {msg}"),
            Self::AlreadyExists(msg) => write!(f, "Already exists: {msg}"),
            Self::InvalidState(msg) => write!(f, "Invalid state: {msg}"),
            Self::UnsupportedDatabase(msg) => write!(f, "Unsupported database: {msg}"),
            Self::UnsupportedType(msg) => write!(f, "Unsupported type: {msg}"),
        }
    }
}

impl std::error::Error for Error {}

// Conversion traits
impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Storage(e.to_string())
    }
}

impl From<tokio_postgres::Error> for Error {
    fn from(e: tokio_postgres::Error) -> Self {
        Error::Query(e.to_string())
    }
}

impl From<deadpool_postgres::PoolError> for Error {
    fn from(e: deadpool_postgres::PoolError) -> Self {
        match e {
            deadpool_postgres::PoolError::Timeout(_) => Error::ConnectionTimeout,
            _ => Error::Connection(e.to_string()),
        }
    }
}

// For Tauri serialization
impl serde::Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;

        let mut state = serializer.serialize_struct("Error", 2)?;
        state.serialize_field("code", &self.code())?;
        state.serialize_field("message", &self.to_string())?;
        state.end()
    }
}

impl Error {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Storage(_) => "STORAGE_ERROR",
            Self::Connection(_) => "CONNECTION_ERROR",
            Self::PoolExhausted => "POOL_EXHAUSTED",
            Self::ConnectionTimeout => "CONNECTION_TIMEOUT",
            Self::Query(_) => "QUERY_ERROR",
            Self::InvalidQuery(_) => "INVALID_QUERY",
            Self::NotFound(_) => "NOT_FOUND",
            Self::AlreadyExists(_) => "ALREADY_EXISTS",
            Self::InvalidState(_) => "INVALID_STATE",
            Self::UnsupportedDatabase(_) => "UNSUPPORTED_DATABASE",
            Self::UnsupportedType(_) => "UNSUPPORTED_TYPE",
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;