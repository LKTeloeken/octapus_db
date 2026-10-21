//! Guards das operações geradas pelo assistente, antes do executor
//! só-leitura (`adapters/read_only.rs`). Falham fechado: linguagem que ainda
//! não tem guard é recusada.

pub mod mongo;
pub mod sql;

use std::fmt;

use crate::adapters::ReadOnlyOp;
use crate::error::{Error, Result};
use crate::models::DatabaseType;

use self::sql::SqlDialect;

/// Por que o guard recusou, na linguagem da operação.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rejection {
    Sql(sql::Rejection),
    Mongo(mongo::Rejection),
}

impl Rejection {
    /// Código estável, para log e para o `reason` do outcome.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Sql(rejection) => rejection.code(),
            Self::Mongo(rejection) => rejection.code(),
        }
    }
}

impl fmt::Display for Rejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sql(rejection) => rejection.fmt(f),
            Self::Mongo(rejection) => rejection.fmt(f),
        }
    }
}

/// Confere a operação para o banco. Recusa → `GUARD_REJECTED`; operação de
/// outra linguagem que a do banco ou ainda sem guard → `UNSUPPORTED_TYPE`.
pub fn check(op: &ReadOnlyOp, db_type: DatabaseType) -> Result<()> {
    match inspect(op, db_type)? {
        None => Ok(()),
        Some(rejection) => {
            log::info!("assistente: consulta recusada pelo guard ({})", rejection.code());
            Err(Error::GuardRejected(rejection.to_string()))
        }
    }
}

/// O motivo da recusa, quando há (o orquestrador manda o código no outcome).
/// `Err` só para operação sem guard (falha fechado).
pub fn inspect(op: &ReadOnlyOp, db_type: DatabaseType) -> Result<Option<Rejection>> {
    match (op, db_type) {
        (ReadOnlyOp::Sql(text), DatabaseType::Postgres) => {
            Ok(sql::check(text, SqlDialect::Postgres).err().map(Rejection::Sql))
        }
        (ReadOnlyOp::Sql(text), DatabaseType::Sqlite) => {
            Ok(sql::check(text, SqlDialect::Sqlite).err().map(Rejection::Sql))
        }
        (ReadOnlyOp::Mongo(text), DatabaseType::Mongodb) => {
            Ok(mongo::check(text).err().map(Rejection::Mongo))
        }
        _ => Err(Error::UnsupportedType(format!(
            "no read-only guard for {:?} on {db_type:?}",
            op.language()
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejection_becomes_guard_rejected() {
        let error = check(&ReadOnlyOp::Sql("DELETE FROM t".into()), DatabaseType::Postgres)
            .unwrap_err();
        assert_eq!(error.code(), "GUARD_REJECTED");
        assert_eq!(
            error.to_string(),
            "Guard rejected: only read queries (SELECT) are allowed"
        );
        assert!(check(&ReadOnlyOp::Sql("SELECT 1".into()), DatabaseType::Sqlite).is_ok());

        let error = check(
            &ReadOnlyOp::Mongo(r#"{"collection":"t","operation":"aggregate","pipeline":[{"$out":"x"}]}"#.into()),
            DatabaseType::Mongodb,
        )
        .unwrap_err();
        assert_eq!(error.code(), "GUARD_REJECTED");
        assert_eq!(error.to_string(), "Guard rejected: $out writes to the database and is not allowed");
        assert!(check(
            &ReadOnlyOp::Mongo(r#"{"collection":"t","operation":"find"}"#.into()),
            DatabaseType::Mongodb
        )
        .is_ok());
    }

    #[test]
    fn languages_without_guard_fail_closed() {
        for (op, db_type) in [
            (ReadOnlyOp::Mongo("{}".into()), DatabaseType::Postgres),
            (ReadOnlyOp::RedisPlan("{}".into()), DatabaseType::Redis),
            (ReadOnlyOp::Sql("SELECT 1".into()), DatabaseType::Mongodb),
        ] {
            assert_eq!(check(&op, db_type).unwrap_err().code(), "UNSUPPORTED_TYPE");
        }
    }
}
