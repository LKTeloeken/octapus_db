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

#[cfg_attr(not(test), allow(dead_code))]
const POSTGRES_V1: Prompt = Prompt {
    version: "postgres-v1",
    paradigm: "postgres",
    text: include_str!("../../prompts/postgres-v1.txt"),
};

#[cfg_attr(not(test), allow(dead_code))]
const SQLITE_V1: Prompt = Prompt {
    version: "sqlite-v1",
    paradigm: "sqlite",
    text: include_str!("../../prompts/sqlite-v1.txt"),
};

#[cfg_attr(not(test), allow(dead_code))]
const MONGO_V1: Prompt = Prompt {
    version: "mongo-v1",
    paradigm: "mongodb",
    text: include_str!("../../prompts/mongo-v1.txt"),
};

#[cfg_attr(not(test), allow(dead_code))]
const REDIS_V1: Prompt = Prompt {
    version: "redis-v1",
    paradigm: "redis",
    text: include_str!("../../prompts/redis-v1.txt"),
};

// v2 (contrato 1.5.0): o modelo pode pedir mais estrutura (`structure_request`)
// e responder só em texto (`answer`), sem afirmar o que não viu e recusando o
// que não é de banco de dados. Os v1 ficam: as conversas antigas foram
// geradas com eles, e a API continua a conhecê-los.

#[cfg_attr(not(test), allow(dead_code))]
const POSTGRES_V2: Prompt = Prompt {
    version: "postgres-v2",
    paradigm: "postgres",
    text: include_str!("../../prompts/postgres-v2.txt"),
};

#[cfg_attr(not(test), allow(dead_code))]
const SQLITE_V2: Prompt = Prompt {
    version: "sqlite-v2",
    paradigm: "sqlite",
    text: include_str!("../../prompts/sqlite-v2.txt"),
};

#[cfg_attr(not(test), allow(dead_code))]
const MONGO_V2: Prompt = Prompt {
    version: "mongo-v2",
    paradigm: "mongodb",
    text: include_str!("../../prompts/mongo-v2.txt"),
};

const REDIS_V2: Prompt = Prompt {
    version: "redis-v2",
    paradigm: "redis",
    text: include_str!("../../prompts/redis-v2.txt"),
};

// v3 (mesmo contrato 1.5.0): no pedido de estrutura, nomes com prefixo —
// `indexes:`, `functions:` (Postgres) e `definition:` — para os índices, as
// funções e a definição de views, funções e índices, com os literais trocados
// por `?` (`assistant::literals`). O Redis não tem nada disso e fica no v2.

#[cfg_attr(not(test), allow(dead_code))]
const POSTGRES_V3: Prompt = Prompt {
    version: "postgres-v3",
    paradigm: "postgres",
    text: include_str!("../../prompts/postgres-v3.txt"),
};

#[cfg_attr(not(test), allow(dead_code))]
const SQLITE_V3: Prompt = Prompt {
    version: "sqlite-v3",
    paradigm: "sqlite",
    text: include_str!("../../prompts/sqlite-v3.txt"),
};

#[cfg_attr(not(test), allow(dead_code))]
const MONGO_V3: Prompt = Prompt {
    version: "mongo-v3",
    paradigm: "mongodb",
    text: include_str!("../../prompts/mongo-v3.txt"),
};

// v4 (mesmo contrato): o SQL vem em várias linhas, uma cláusula por linha, para
// ler na tela. No Mongo e no Redis a consulta é JSON e o app já indenta
// (`formatQueryText`): ficam no v3 e no v2.

#[cfg_attr(not(test), allow(dead_code))]
const POSTGRES_V4: Prompt = Prompt {
    version: "postgres-v4",
    paradigm: "postgres",
    text: include_str!("../../prompts/postgres-v4.txt"),
};

#[cfg_attr(not(test), allow(dead_code))]
const SQLITE_V4: Prompt = Prompt {
    version: "sqlite-v4",
    paradigm: "sqlite",
    text: include_str!("../../prompts/sqlite-v4.txt"),
};

// v5 do SQL (mesmo contrato): `columns:schema.*termo*` acha as colunas pelo
// nome (o "onde está o CPF?"), e os comentários de schema e de função vão no
// texto. v4 do Mongo: os valores de exemplo dos campos categóricos (quando o
// servidor liga) e as descrições do `$jsonSchema`.

const POSTGRES_V5: Prompt = Prompt {
    version: "postgres-v5",
    paradigm: "postgres",
    text: include_str!("../../prompts/postgres-v5.txt"),
};

const SQLITE_V5: Prompt = Prompt {
    version: "sqlite-v5",
    paradigm: "sqlite",
    text: include_str!("../../prompts/sqlite-v5.txt"),
};

const MONGO_V4: Prompt = Prompt {
    version: "mongo-v4",
    paradigm: "mongodb",
    text: include_str!("../../prompts/mongo-v4.txt"),
};

/// Todos os prompts que este build conhece.
#[cfg(test)]
pub const ALL: [Prompt; 16] = [
    POSTGRES_V1,
    SQLITE_V1,
    MONGO_V1,
    REDIS_V1,
    POSTGRES_V2,
    SQLITE_V2,
    MONGO_V2,
    REDIS_V2,
    POSTGRES_V3,
    SQLITE_V3,
    MONGO_V3,
    POSTGRES_V4,
    SQLITE_V4,
    POSTGRES_V5,
    SQLITE_V5,
    MONGO_V4,
];

/// O prompt em uso para o banco. `None` = o assistente não atende o
/// paradigma.
pub fn for_db(db_type: DatabaseType) -> Option<Prompt> {
    match db_type {
        DatabaseType::Postgres => Some(POSTGRES_V5),
        DatabaseType::Sqlite => Some(SQLITE_V5),
        DatabaseType::Mongodb => Some(MONGO_V4),
        DatabaseType::Redis => Some(REDIS_V2),
        DatabaseType::Mysql => None,
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

    impl Prompt {
        /// A versão sem o `-v<n>` do fim
        fn name(&self) -> &'static str {
            self.version.rsplit_once("-v").map_or(self.version, |(name, _)| name)
        }
    }

    /// Os hashes registrados (ou a registrar) no dashboard. Mudou o texto?
    /// Este teste falha: crie uma versão nova do arquivo em vez de editar a
    /// antiga — a API recusaria o texto editado com `prompt_not_registered`.
    const REGISTERED: [(&str, &str); 16] = [
        ("postgres-v1", "283ab7d457abafce41bcb2795c8fe5ae71810da48d7ec3d2bee5c551ec653755"),
        ("sqlite-v1", "6a46cc1011b1a10a7eeb20d5e0601adbb8ba38ec78a78b92038dbc13f3fca73c"),
        ("mongo-v1", "7d7e35fe891ba078e5f741deaec9aa58fd427666c9dad05da88caa54341dfb0e"),
        ("redis-v1", "5341f0feed92e8a452cf2bf3524047600cfdd3ae2c1500ab9bb15331bc9e7279"),
        ("postgres-v2", "d8398abeb66cee293ee83d8cc51eb7cad4b59887395b7e3099da7ad23e044ccc"),
        ("sqlite-v2", "268e1ebbf4348ee66fc5fb40ed36f492cc7cbf63f1db4aebf67bf9147a97479d"),
        ("mongo-v2", "ca9bea2900bda8969599b6030e709c258fba5ab3ab10f9036b4d0763067bf60d"),
        ("redis-v2", "1c016b246ae134f6215cb3d4714e19d2e31b33399b21571c640a888d9e9b94af"),
        ("postgres-v3", "6803ac4792e57a655cc80b269c8366e35d9ab8a61559c5f7a5d85e992960d1ce"),
        ("sqlite-v3", "6e9fbcc46856ada2198b3284765279ba6e66aadb36bb81ebf0c33e5a73199da2"),
        ("mongo-v3", "bbba5042844e2bc9f269bb2f8b1d9c84aec609571c7e65b00ba045e5fc552fe3"),
        ("postgres-v4", "6b645302865e9e96cfb265ebe04c07aaa67604e9e254c8857252721ef552e7e0"),
        ("sqlite-v4", "1f25daf2f8aa42be78a1c9a4cb7470455c917341d21c677a2afa93578dbdac80"),
        ("postgres-v5", "3dd6911343c8dade5aae66e4c9805fd667f12650e48bead226c11f969063b8fc"),
        ("sqlite-v5", "77bb535b295c4531335de02eb5f8ef8bef16ec0ec76eb5329c2034c6c9c8de75"),
        ("mongo-v4", "80331cb55a74c77de2c450f6f230356bea53dd83fae1e43d42415f4619f4a6c2"),
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
                prompt.version,
                prompt.name()
            );
        }
    }

    #[test]
    fn prompts_are_lf_and_name_their_paradigm() {
        for prompt in ALL {
            assert!(!prompt.text.contains('\r'), "{} com CRLF", prompt.version);
            // O nome da versão é o paradigma ou um prefixo dele (`mongo-v1`)
            assert!(prompt.paradigm.starts_with(prompt.name()), "{}", prompt.version);
            assert!(prompt.text.contains("<schema>") && prompt.text.contains("clarificationQuestion"));
        }
        // Os em uso conhecem os tipos do contrato 1.5.0
        for db_type in [DatabaseType::Postgres, DatabaseType::Sqlite, DatabaseType::Mongodb, DatabaseType::Redis] {
            let prompt = for_db(db_type).unwrap();
            for needle in ["structure_request", "structureRequest", "<requested_structure>", "\"answer\"", "limit_reached"] {
                assert!(prompt.text.contains(needle), "{} sem {needle}", prompt.version);
            }
        }
        // Os de banco com catálogo pedem índices e definições (desde o v3), com
        // o `?` dos literais; funções só no Postgres. O Redis segue no v2
        for db_type in [DatabaseType::Postgres, DatabaseType::Sqlite, DatabaseType::Mongodb] {
            let prompt = for_db(db_type).unwrap();
            for needle in ["\"indexes:", "\"definition:", "with ?"] {
                assert!(prompt.text.contains(needle), "{} sem {needle}", prompt.version);
            }
            assert_eq!(
                prompt.text.contains("\"functions:"),
                db_type == DatabaseType::Postgres,
                "{}",
                prompt.version
            );
        }
        // O SQL em várias linhas (desde o v4) e a busca de colunas (v5); Mongo
        // e Redis o app indenta
        for db_type in [DatabaseType::Postgres, DatabaseType::Sqlite] {
            let prompt = for_db(db_type).unwrap();
            assert!(prompt.version.ends_with("-v5"), "{}", prompt.version);
            for needle in ["Lay the SQL out on several lines", "\"columns:"] {
                assert!(prompt.text.contains(needle), "{} sem {needle}", prompt.version);
            }
        }
        // Mongo v4: os valores de exemplo e as descrições do validador
        let mongo = for_db(DatabaseType::Mongodb).unwrap();
        assert_eq!(mongo.version, "mongo-v4");
        assert!(mongo.text.contains("values:") && mongo.text.contains("$jsonSchema"));
        assert!(!mongo.text.contains("\"columns:"));
        assert_eq!(for_db(DatabaseType::Redis).unwrap().version, "redis-v2");
        assert_eq!(for_db(DatabaseType::Postgres).unwrap().paradigm, "postgres");
        assert_eq!(for_db(DatabaseType::Sqlite).unwrap().paradigm, "sqlite");
        assert_eq!(for_db(DatabaseType::Mongodb).unwrap().paradigm, "mongodb");
        assert_eq!(for_db(DatabaseType::Redis).unwrap().paradigm, "redis");
        assert_eq!(paradigm(DatabaseType::Mongodb), Some("mongodb"));
    }

    /// Banco que o assistente atende (capabilities) tem prompt e paradigma.
    #[test]
    fn every_assistant_database_has_a_prompt() {
        use crate::models::AdapterCapabilities;

        for entry in AdapterCapabilities::all() {
            if entry.capabilities.supports_assistant {
                let prompt = for_db(entry.db_type).unwrap_or_else(|| panic!("{:?} sem prompt", entry.db_type));
                assert_eq!(Some(prompt.paradigm), paradigm(entry.db_type));
            }
        }
    }
}
