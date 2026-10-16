use serde::{Deserialize, Serialize};

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DatabaseType {
    Postgres,
    /// Sem adapter: nenhum servidor chega ao front com ele
    #[cfg_attr(test, ts(skip))]
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

/// Como a conexão usa TLS — os nomes e a semântica do `sslmode` do libpq.
///
/// Postgres aceita os cinco. Mongo e Redis só `disable`, `require` e
/// `verify-full` (com rustls, nenhum dos dois checa só a CA nem cai para texto
/// puro): o que chega fora disso é ajustado por [`SslMode::for_db_type`] ao
/// gravar. SQLite é arquivo local, sempre `disable`.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SslMode {
    /// Sem TLS
    #[default]
    Disable,
    /// TLS se o servidor aceitar, senão texto puro; sem verificar o certificado
    Prefer,
    /// TLS obrigatório, sem verificar o certificado (servidor autoassinado)
    Require,
    /// TLS obrigatório; o certificado precisa vir de uma CA confiável
    VerifyCa,
    /// Como `VerifyCa` e o nome no certificado precisa bater com o host
    VerifyFull,
}

impl SslMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disable => "disable",
            Self::Prefer => "prefer",
            Self::Require => "require",
            Self::VerifyCa => "verify-ca",
            Self::VerifyFull => "verify-full",
        }
    }

    /// Nome do libpq (ou de outro app), sem diferenciar caixa. `allow` vira
    /// `prefer`: os dois aceitam texto puro, e o app não tenta texto primeiro.
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().replace('_', "-").as_str() {
            "disable" | "disabled" | "off" | "false" => Some(Self::Disable),
            "allow" | "prefer" | "preferred" => Some(Self::Prefer),
            "require" | "required" | "on" | "true" => Some(Self::Require),
            "verify-ca" => Some(Self::VerifyCa),
            "verify-full" | "verify-identity" => Some(Self::VerifyFull),
            _ => None,
        }
    }

    /// O modo mais próximo que o banco suporta (ver a doc do tipo). Na
    /// dúvida, fica o mais seguro que ainda conecta: `verify-ca` sobe para
    /// `verify-full`; `prefer` — que nunca garantiu TLS — vira `disable`.
    pub fn for_db_type(self, db_type: DatabaseType) -> Self {
        match db_type {
            DatabaseType::Postgres | DatabaseType::Mysql => self,
            DatabaseType::Sqlite => Self::Disable,
            DatabaseType::Mongodb | DatabaseType::Redis => match self {
                Self::Prefer => Self::Disable,
                Self::VerifyCa => Self::VerifyFull,
                other => other,
            },
        }
    }

    pub fn uses_tls(self) -> bool {
        self != Self::Disable
    }

    /// Verifica a cadeia do certificado (os dois `verify-*`)
    pub fn verifies(self) -> bool {
        matches!(self, Self::VerifyCa | Self::VerifyFull)
    }
}

#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Server {
    /// Sempre preenchido no que sai do storage para o front
    #[cfg_attr(test, ts(type = "number"))]
    pub id: Option<i64>,
    pub name: String,
    pub db_type: DatabaseType,
    pub host: String,
    pub port: u16,
    pub username: String,
    #[serde(skip_serializing)] // Never send password to frontend
    #[cfg_attr(test, ts(skip))]
    pub password: String,
    pub default_database: Option<String>,
    #[serde(default)]
    pub ssl_mode: SslMode,
    /// CA própria em PEM (caminho do arquivo) para os modos `verify-*`; sem
    /// ela, valem as CAs confiáveis do sistema
    #[serde(default)]
    pub ssl_root_cert: Option<String>,
    /// Full connection URI (e.g. mongodb+srv://... or rediss://...); when
    /// present it takes precedence over host/port/username for connecting.
    pub connection_uri: Option<String>,
    pub created_at: i64,
    /// Escopo salvo: que databases o app enxerga neste servidor (padrões com
    /// `*`, `?` e `!` para excluir — ver `catalog::NameScope`)
    #[serde(default)]
    pub scope_databases: Option<String>,
    /// Escopo salvo: que schemas o catálogo lê (Postgres)
    #[serde(default)]
    pub scope_schemas: Option<String>,
    /// Grupo da sidebar (`server_groups`); `None` = fora de qualquer grupo
    #[serde(default)]
    pub group_id: Option<i64>,
}

/// For creating/updating servers (receives password)
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
// Campos `Option` podem faltar no JSON (viram None)
#[cfg_attr(test, ts(optional_fields = nullable))]
pub struct ServerInput {
    pub name: String,
    pub db_type: DatabaseType,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub default_database: Option<String>,
    #[serde(default)]
    pub ssl_mode: Option<SslMode>,
    #[serde(default)]
    pub ssl_root_cert: Option<String>,
    pub connection_uri: Option<String>,
    #[serde(default)]
    pub scope_databases: Option<String>,
    #[serde(default)]
    pub scope_schemas: Option<String>,
    #[serde(default)]
    pub group_id: Option<i64>,
}

/// Grupo de servidores na sidebar (pasta de um nível). Excluir o grupo não
/// exclui os servidores: eles voltam para fora de qualquer grupo.
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServerGroup {
    pub id: i64,
    pub name: String,
    pub created_at: i64,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_names_from_libpq_and_other_apps() {
        assert_eq!(SslMode::parse("verify-full"), Some(SslMode::VerifyFull));
        assert_eq!(SslMode::parse("VERIFY_CA"), Some(SslMode::VerifyCa));
        assert_eq!(SslMode::parse("allow"), Some(SslMode::Prefer));
        assert_eq!(SslMode::parse("verify-identity"), Some(SslMode::VerifyFull));
        assert_eq!(SslMode::parse("required"), Some(SslMode::Require));
        assert_eq!(SslMode::parse("sei-lá"), None);
        for mode in [SslMode::Disable, SslMode::Prefer, SslMode::Require, SslMode::VerifyCa, SslMode::VerifyFull] {
            assert_eq!(SslMode::parse(mode.as_str()), Some(mode));
            // O JSON usa os mesmos nomes
            assert_eq!(serde_json::to_value(mode).unwrap(), mode.as_str());
        }
    }

    #[test]
    fn adjusts_to_what_each_database_supports() {
        use DatabaseType::*;
        assert_eq!(SslMode::VerifyCa.for_db_type(Postgres), SslMode::VerifyCa);
        assert_eq!(SslMode::Prefer.for_db_type(Postgres), SslMode::Prefer);
        for db in [Mongodb, Redis] {
            assert_eq!(SslMode::VerifyCa.for_db_type(db), SslMode::VerifyFull);
            assert_eq!(SslMode::Prefer.for_db_type(db), SslMode::Disable);
            assert_eq!(SslMode::Require.for_db_type(db), SslMode::Require);
        }
        assert_eq!(SslMode::VerifyFull.for_db_type(Sqlite), SslMode::Disable);
    }
}
