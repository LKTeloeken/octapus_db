use std::fmt;

use crate::assistant::AssistantError;

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
    /// Erro do banco numa consulta do assistente, com o SQLSTATE do Postgres
    /// para classificar e sanitizar a falha (`assistant/failure.rs`). Para o
    /// front é igual ao `Query`: mesmo código, mesma mensagem.
    DbQuery {
        sql_state: String,
        message: String,
    },
    /// Erro do servidor Mongo numa consulta do assistente, com o código
    /// numérico e o `codeName` para classificar e sanitizar. Para o front é
    /// igual ao `Query`.
    MongoQuery {
        code: i32,
        code_name: String,
        message: String,
    },

    // Resource errors
    NotFound(String),
    AlreadyExists(String),

    // State errors
    InvalidState(String),

    /// A operação foi cancelada a pedido (não é falha do banco)
    Cancelled,

    // Adapter errors
    UnsupportedDatabase(String),
    UnsupportedType(String),

    /// Falha fora dos bancos: ferramentas externas (pg_dump…), sistema de
    /// arquivos, API do updater. A mensagem já é escrita para o usuário, então
    /// sai sem prefixo.
    External(String),

    /// O guard do assistente recusou a consulta antes de ela ir ao banco
    /// (`assistant/guard/`): escrita, mais de um statement, função proibida…
    GuardRejected(String),

    /// Assistente de consulta sem a URL da API (`assistant/config.rs`).
    AssistantNotConfigured,
    /// API do assistente fora do ar ou erro devolvido por ela
    /// (`assistant/error.rs`): vira um código `ASSISTANT_*`.
    Assistant(AssistantError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(msg) => write!(f, "Storage error: {msg}"),
            Self::Connection(msg) => write!(f, "Connection error: {msg}"),
            Self::PoolExhausted => write!(f, "Connection pool exhausted"),
            Self::ConnectionTimeout => write!(f, "Connection timed out"),
            Self::Query(msg)
            | Self::DbQuery { message: msg, .. }
            | Self::MongoQuery { message: msg, .. } => {
                write!(f, "Query error: {msg}")
            }
            Self::InvalidQuery(msg) => write!(f, "Invalid query: {msg}"),
            Self::NotFound(msg) => write!(f, "Not found: {msg}"),
            Self::AlreadyExists(msg) => write!(f, "Already exists: {msg}"),
            Self::InvalidState(msg) => write!(f, "Invalid state: {msg}"),
            Self::Cancelled => write!(f, "Operation cancelled"),
            Self::UnsupportedDatabase(msg) => write!(f, "Unsupported database: {msg}"),
            Self::UnsupportedType(msg) => write!(f, "Unsupported type: {msg}"),
            Self::External(msg) => f.write_str(msg),
            Self::GuardRejected(msg) => write!(f, "Guard rejected: {msg}"),
            Self::AssistantNotConfigured => write!(f, "Assistant not configured"),
            Self::Assistant(error) => {
                write!(f, "Assistant {}: {}", error.code.api_name(), error.message)
            }
        }
    }
}

impl std::error::Error for Error {}

impl From<AssistantError> for Error {
    fn from(e: AssistantError) -> Self {
        Error::Assistant(e)
    }
}

// Conversion traits
impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Storage(e.to_string())
    }
}

/// A mensagem com as causas encadeadas (`a: b: c`). O tokio-postgres diz só
/// "error performing TLS handshake"; o motivo (CA desconhecida, nome que não
/// bate) está no `source()`.
pub fn error_chain(error: &dyn std::error::Error) -> String {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        let text = cause.to_string();
        if !message.contains(&text) {
            message.push_str(": ");
            message.push_str(&text);
        }
        source = cause.source();
    }
    message
}

impl From<tokio_postgres::Error> for Error {
    fn from(e: tokio_postgres::Error) -> Self {
        // tokio_postgres Display is just "db error"; the server message
        // (e.g. "canceling statement due to user request") lives in DbError.
        match e.as_db_error() {
            Some(db) => Error::Query(db.message().to_string()),
            None => Error::Query(error_chain(&e)),
        }
    }
}

impl From<mongodb::error::Error> for Error {
    fn from(e: mongodb::error::Error) -> Self {
        use mongodb::error::ErrorKind;
        match *e.kind {
            ErrorKind::ServerSelection { .. } | ErrorKind::DnsResolve { .. } => {
                Error::Connection(e.to_string())
            }
            ErrorKind::Authentication { .. } => Error::Connection(e.to_string()),
            _ => Error::Query(e.to_string()),
        }
    }
}

impl From<redis::RedisError> for Error {
    fn from(e: redis::RedisError) -> Self {
        if e.is_connection_refusal() || e.is_connection_dropped() || e.is_timeout() {
            Error::Connection(e.to_string())
        } else {
            Error::Query(e.to_string())
        }
    }
}

impl From<deadpool_postgres::PoolError> for Error {
    fn from(e: deadpool_postgres::PoolError) -> Self {
        match e {
            deadpool_postgres::PoolError::Timeout(_) => Error::ConnectionTimeout,
            _ => Error::Connection(error_chain(&e)),
        }
    }
}

/// Formato da rejeição no front: `{ code, message }`, com `message` igual ao
/// `Display` (ver `ApiError` em src/api/client.ts).
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
            Self::Query(_) | Self::DbQuery { .. } | Self::MongoQuery { .. } => "QUERY_ERROR",
            Self::InvalidQuery(_) => "INVALID_QUERY",
            Self::NotFound(_) => "NOT_FOUND",
            Self::AlreadyExists(_) => "ALREADY_EXISTS",
            Self::InvalidState(_) => "INVALID_STATE",
            Self::Cancelled => "CANCELLED",
            Self::UnsupportedDatabase(_) => "UNSUPPORTED_DATABASE",
            Self::UnsupportedType(_) => "UNSUPPORTED_TYPE",
            Self::External(_) => "EXTERNAL_ERROR",
            Self::GuardRejected(_) => "GUARD_REJECTED",
            Self::AssistantNotConfigured => "ASSISTANT_NOT_CONFIGURED",
            Self::Assistant(error) => error.code.app_code(),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_code_and_display_message() {
        let json =
            serde_json::to_value(Error::Query("relation \"x\" does not exist".into())).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "code": "QUERY_ERROR",
                "message": "Query error: relation \"x\" does not exist",
            })
        );
    }

    #[test]
    fn external_message_has_no_prefix() {
        let error = Error::External("O pg_dump não foi encontrado.".into());
        assert_eq!(error.to_string(), "O pg_dump não foi encontrado.");
        assert_eq!(error.code(), "EXTERNAL_ERROR");
    }
}
