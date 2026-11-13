//! Os erros do `octapus` e o código de saída de cada um.
//!
//! O erro vai para o stderr: `{ "code", "message" }` em JSON quando a saída é
//! para programa, uma linha quando é para gente. O código de saída diz o tipo
//! sem precisar ler o texto.

use std::fmt;

use octapus_core::Error as CoreError;

#[derive(Debug)]
pub enum CliError {
    /// Um erro do núcleo, com o código dele (`CONNECTION_ERROR`, `QUERY_ERROR`…)
    Core(CoreError),
    /// Pedido que não fecha (argumento que falta, nome ambíguo)
    Usage(String),
    /// Escrita pedida sem um terminal na frente, ou não confirmada
    WriteRefused(String),
    /// Falha ao ler ou escrever (arquivo, entrada padrão)
    Io(String),
    /// O programa do outro lado do pipe fechou (`octapus … | head`): não é erro
    BrokenPipe,
}

pub type CliResult<T> = Result<T, CliError>;

impl CliError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Core(error) => error.code(),
            Self::Usage(_) => "USAGE",
            Self::WriteRefused(_) => "WRITE_REFUSED",
            Self::Io(_) => "IO_ERROR",
            Self::BrokenPipe => "BROKEN_PIPE",
        }
    }

    pub fn exit_code(&self) -> u8 {
        match self.code() {
            "BROKEN_PIPE" => 0,
            "USAGE" => 2,
            "NOT_FOUND" => 3,
            "CONNECTION_ERROR" | "CONNECTION_TIMEOUT" | "POOL_EXHAUSTED" => 4,
            "GUARD_REJECTED" => 5,
            "QUERY_ERROR" | "INVALID_QUERY" => 6,
            "UNSUPPORTED_DATABASE" | "UNSUPPORTED_TYPE" => 7,
            "WRITE_REFUSED" => 8,
            "CANCELLED" => 130,
            _ => 1,
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Core(error) => error.fmt(f),
            Self::Usage(message) | Self::WriteRefused(message) | Self::Io(message) => f.write_str(message),
            Self::BrokenPipe => f.write_str("broken pipe"),
        }
    }
}

impl From<CoreError> for CliError {
    fn from(error: CoreError) -> Self {
        Self::Core(error)
    }
}

impl From<std::io::Error> for CliError {
    fn from(error: std::io::Error) -> Self {
        if error.kind() == std::io::ErrorKind::BrokenPipe {
            Self::BrokenPipe
        } else {
            Self::Io(error.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_follow_the_error_kind() {
        assert_eq!(CliError::Core(CoreError::NotFound("x".into())).exit_code(), 3);
        assert_eq!(CliError::Core(CoreError::ConnectionTimeout).exit_code(), 4);
        assert_eq!(CliError::Core(CoreError::GuardRejected("x".into())).exit_code(), 5);
        assert_eq!(CliError::Core(CoreError::Query("x".into())).exit_code(), 6);
        assert_eq!(CliError::Core(CoreError::UnsupportedType("x".into())).exit_code(), 7);
        assert_eq!(CliError::WriteRefused("x".into()).exit_code(), 8);
        assert_eq!(CliError::Core(CoreError::Cancelled).exit_code(), 130);
        assert_eq!(CliError::Usage("x".into()).exit_code(), 2);
        assert_eq!(CliError::Core(CoreError::Storage("x".into())).exit_code(), 1);
        assert_eq!(CliError::BrokenPipe.exit_code(), 0);
    }
}
