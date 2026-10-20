//! Guards das operações geradas pelo assistente, antes do executor
//! só-leitura (`adapters/read_only.rs`). Falham fechado: linguagem que ainda
//! não tem guard é recusada.

pub mod sql;

use crate::adapters::ReadOnlyOp;
use crate::error::{Error, Result};
use crate::models::DatabaseType;

use self::sql::SqlDialect;

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
pub fn inspect(op: &ReadOnlyOp, db_type: DatabaseType) -> Result<Option<sql::Rejection>> {
    let dialect = match (op, db_type) {
        (ReadOnlyOp::Sql(_), DatabaseType::Postgres) => SqlDialect::Postgres,
        (ReadOnlyOp::Sql(_), DatabaseType::Sqlite) => SqlDialect::Sqlite,
        _ => {
            return Err(Error::UnsupportedType(format!(
                "no read-only guard for {:?} on {db_type:?}",
                op.language()
            )))
        }
    };
    let ReadOnlyOp::Sql(text) = op else {
        unreachable!("só SQL chega aqui");
    };

    Ok(sql::check(text, dialect).err())
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
    }

    #[test]
    fn languages_without_guard_fail_closed() {
        for (op, db_type) in [
            (ReadOnlyOp::Mongo("{}".into()), DatabaseType::Mongodb),
            (ReadOnlyOp::RedisPlan("{}".into()), DatabaseType::Redis),
            (ReadOnlyOp::Sql("SELECT 1".into()), DatabaseType::Mongodb),
        ] {
            assert_eq!(check(&op, db_type).unwrap_err().code(), "UNSUPPORTED_TYPE");
        }
    }
}
