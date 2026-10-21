//! Os system prompts do assistente, um por paradigma, versionados em
//! `src-tauri/prompts/` e embutidos no build.
//!
//! A API confere o SHA-256 dos **bytes exatos** da mensagem `system` contra o
//! prompt registrado no dashboard (Prompts → enviar arquivo), com o mesmo id.
//! Então: registre o próprio arquivo daqui, e qualquer mudança no texto é uma
//! versão nova (`postgres-v2.txt`), registrada antes de o app sair. O
//! `.gitattributes` fixa LF, para o checkout do Windows não mudar o hash.
//!
//! Para ver os hashes: `cargo test --lib assistant::prompts -- --nocapture`.

use crate::models::DatabaseType;

/// Um system prompt registrado na API.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Prompt {
    /// O `promptVersion` do pedido de tentativa (o id no dashboard)
    pub version: &'static str,
    /// Paradigma do contrato (`postgres`, `sqlite`…)
    pub paradigm: &'static str,
    pub text: &'static str,
}

#[cfg(test)]
impl Prompt {
    /// SHA-256 em hexadecimal, o mesmo que o dashboard mostra.
    pub fn sha256(&self) -> String {
        use sha2::{Digest, Sha256};
        hex::encode(Sha256::digest(self.text.as_bytes()))
    }
}

const POSTGRES_V1: Prompt = Prompt {
    version: "postgres-v1",
    paradigm: "postgres",
    text: include_str!("../../prompts/postgres-v1.txt"),
};

const SQLITE_V1: Prompt = Prompt {
    version: "sqlite-v1",
    paradigm: "sqlite",
    text: include_str!("../../prompts/sqlite-v1.txt"),
};

const MONGODB_V1: Prompt = Prompt {
    version: "mongodb-v1",
    paradigm: "mongodb",
    text: include_str!("../../prompts/mongodb-v1.txt"),
};

/// Todos os prompts que este build conhece.
#[cfg(test)]
pub const ALL: [Prompt; 3] = [POSTGRES_V1, SQLITE_V1, MONGODB_V1];

/// O prompt em uso para o banco. `None` = o assistente ainda não atende o
/// paradigma (o Redis entra com a parte B9).
pub fn for_db(db_type: DatabaseType) -> Option<Prompt> {
    match db_type {
        DatabaseType::Postgres => Some(POSTGRES_V1),
        DatabaseType::Sqlite => Some(SQLITE_V1),
        DatabaseType::Mongodb => Some(MONGODB_V1),
        _ => None,
    }
}

/// O paradigma do contrato para o banco (o `paradigm` da conversa).
pub fn paradigm(db_type: DatabaseType) -> Option<&'static str> {
    match db_type {
        DatabaseType::Postgres => Some("postgres"),
        DatabaseType::Sqlite => Some("sqlite"),
        DatabaseType::Mongodb => Some("mongodb"),
        DatabaseType::Redis => Some("redis"),
        DatabaseType::Mysql => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Os hashes registrados (ou a registrar) no dashboard. Mudou o texto?
    /// Este teste falha: crie uma versão nova do arquivo em vez de editar a
    /// antiga — a API recusaria o texto editado com `prompt_not_registered`.
    const REGISTERED: [(&str, &str); 3] = [
        ("postgres-v1", "283ab7d457abafce41bcb2795c8fe5ae71810da48d7ec3d2bee5c551ec653755"),
        ("sqlite-v1", "6a46cc1011b1a10a7eeb20d5e0601adbb8ba38ec78a78b92038dbc13f3fca73c"),
        ("mongodb-v1", "7d7e35fe891ba078e5f741deaec9aa58fd427666c9dad05da88caa54341dfb0e"),
    ];

    #[test]
    fn prints_and_pins_each_prompt_hash() {
        for prompt in ALL {
            let sha = prompt.sha256();
            println!("{}\t{}\t{sha}\t{} bytes", prompt.version, prompt.paradigm, prompt.text.len());
            let registered = REGISTERED
                .iter()
                .find(|(version, _)| *version == prompt.version)
                .unwrap_or_else(|| panic!("{} sem hash registrado no teste", prompt.version));
            assert_eq!(
                sha, registered.1,
                "{} mudou: crie {}-v<n+1>.txt e registre no dashboard",
                prompt.version, prompt.paradigm
            );
        }
    }

    #[test]
    fn prompts_are_lf_and_name_their_paradigm() {
        for prompt in ALL {
            assert!(!prompt.text.contains('\r'), "{} com CRLF", prompt.version);
            assert!(prompt.version.starts_with(prompt.paradigm), "{}", prompt.version);
            assert!(prompt.text.contains("<schema>") && prompt.text.contains("clarificationQuestion"));
        }
        assert_eq!(for_db(DatabaseType::Postgres).unwrap().paradigm, "postgres");
        assert_eq!(for_db(DatabaseType::Sqlite).unwrap().paradigm, "sqlite");
        assert_eq!(for_db(DatabaseType::Mongodb).unwrap().paradigm, "mongodb");
        assert!(for_db(DatabaseType::Redis).is_none());
        assert_eq!(paradigm(DatabaseType::Mongodb), Some("mongodb"));
    }
}
