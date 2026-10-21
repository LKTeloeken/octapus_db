//! Execução só-leitura das consultas geradas pelo assistente (RN-16).
//!
//! A barreira do banco vale sozinha, sem depender do guard
//! (`assistant/guard/`): um statement só (protocolo estendido no Postgres,
//! `prepare` no SQLite), transação `READ ONLY` desfeita no fim (Postgres) ou
//! `PRAGMA query_only` (SQLite), tempo e linhas com teto. No Mongo, que não
//! tem transação de leitura, o comando sai do tipo da operação e o pipeline
//! que escreve (`$out`/`$merge`) é recusado no executor (`mongo/read_only.rs`).

use std::time::Duration;

use crate::models::QueryLanguage;

/// Uma operação de leitura, na linguagem do banco (o `language` da saída do
/// modelo no contrato da API).
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)] // RedisPlan entra com a parte B9
pub enum ReadOnlyOp {
    /// Uma consulta SQL (Postgres e SQLite)
    Sql(String),
    /// Operação do Mongo: o JSON do `mongo-operation.schema.json`
    Mongo(String),
    /// Plano do Redis: o JSON do `redis-plan.schema.json`
    RedisPlan(String),
}

impl ReadOnlyOp {
    /// O texto da consulta gerada, na linguagem do banco (`query_language`
    /// das capabilities).
    pub fn new(language: QueryLanguage, text: impl Into<String>) -> Self {
        let text = text.into();
        match language {
            QueryLanguage::Sql => Self::Sql(text),
            QueryLanguage::Mongo => Self::Mongo(text),
            QueryLanguage::RedisPlan => Self::RedisPlan(text),
        }
    }

    pub fn language(&self) -> QueryLanguage {
        match self {
            Self::Sql(_) => QueryLanguage::Sql,
            Self::Mongo(_) => QueryLanguage::Mongo,
            Self::RedisPlan(_) => QueryLanguage::RedisPlan,
        }
    }
}

/// Tetos de uma execução só-leitura.
#[derive(Debug, Clone, Copy)]
pub struct ReadOnlyLimits {
    /// Tempo da consulta (`statement_timeout` no Postgres; no SQLite, a
    /// consulta é interrompida).
    pub statement_timeout: Duration,
    /// Espera por lock (Postgres): uma leitura nunca fica presa atrás de uma
    /// migração.
    pub lock_timeout: Duration,
    /// Linhas por página, qualquer que seja o `limit` pedido.
    pub max_rows: i64,
}

impl Default for ReadOnlyLimits {
    fn default() -> Self {
        Self {
            statement_timeout: Duration::from_secs(30),
            lock_timeout: Duration::from_secs(2),
            max_rows: 1000,
        }
    }
}

impl ReadOnlyLimits {
    /// `limit` e `offset` da página, dentro do teto.
    pub fn page(&self, limit: i64, offset: i64) -> (i64, i64) {
        (limit.clamp(1, self.max_rows), offset.max(0))
    }
}

/// A mensagem de tempo esgotado, igual à do Postgres (SQLSTATE 57014), para o
/// classificador tratar os dois bancos do mesmo jeito.
pub const STATEMENT_TIMEOUT_MESSAGE: &str = "canceling statement due to statement timeout";

/// Tira espaços e `;` do fim: a consulta vai dentro de uma subquery.
pub fn trim_statement(sql: &str) -> &str {
    let mut sql = sql.trim();
    while let Some(rest) = sql.strip_suffix(';') {
        sql = rest.trim_end();
    }
    sql
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_stays_within_the_cap() {
        let limits = ReadOnlyLimits::default();
        assert_eq!(limits.page(500, 0), (500, 0));
        assert_eq!(limits.page(1_000_000, 20), (1000, 20));
        assert_eq!(limits.page(0, -5), (1, 0));
    }

    #[test]
    fn trims_trailing_semicolons() {
        assert_eq!(trim_statement("  SELECT 1 ;; \n"), "SELECT 1");
        assert_eq!(trim_statement("SELECT ';'"), "SELECT ';'");
    }
}
