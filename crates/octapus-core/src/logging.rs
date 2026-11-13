//! A política de níveis do log, igual no app e no terminal: o que é nosso
//! loga em `info` (em dev, `debug`) e as dependências só a partir de `warn`.
//!
//! O que vai para o log serve para diagnosticar um relato de bug, então tem
//! regra: **nunca** senha, URI de conexão, texto de query nem nomes de schema
//! ou tabela. Servidor pelo id e database pelo nome bastam; mensagens de erro
//! do banco entram como vieram.

use log::LevelFilter;

/// O target dos logs do núcleo (o nome desta crate).
pub const CORE_TARGET: &str = "octapus_core";
/// As dependências só a partir de `warn` — o `debug` do tokio-postgres, por
/// exemplo, traz o SQL.
pub const DEPENDENCIES_LEVEL: LevelFilter = LevelFilter::Warn;

/// Em dev o app loga em `debug`; em release, `info`.
pub fn app_level() -> LevelFilter {
    if cfg!(debug_assertions) {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    }
}
