//! Do que o app de origem descreve (`RawConnection`) para o Octapus: o
//! candidato da prévia (sem senha) e o `ServerInput` do commit. Rust puro.

use std::path::Path;

use crate::models::{DatabaseType, ImportCandidate, ImportWarning, Server, ServerInput};

/// Onde está a senha de uma conexão lida.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Secret {
    None,
    /// Já em mãos: arquivo decifrado, URL ou `.pgpass`
    Plain(String),
    /// No Keychain do macOS — lida só no commit (cada leitura pede permissão)
    Keychain {
        service: &'static str,
        account: String,
    },
}

/// Uma conexão como o app de origem a descreve, antes de virar do Octapus.
#[derive(Debug, Clone)]
pub struct RawConnection {
    pub name: String,
    /// Banco no app de origem, já normalizado: "PostgreSQL", "MongoDB",
    /// "Redis", "SQLite" ou o nome de um banco sem suporte ("MySQL"…)
    pub driver: String,
    pub host: String,
    pub port: Option<u16>,
    pub username: String,
    /// Database padrão; no SQLite, o caminho do arquivo
    pub database: Option<String>,
    /// URI completa, quando só ela descreve a conexão (SRV, vários hosts…)
    pub uri: Option<String>,
    /// `sslmode` no vocabulário do libpq (`disable`, `prefer`, `require`,
    /// `verify-ca`, `verify-full`); `None` = sem SSL
    pub ssl_mode: Option<String>,
    pub client_certificate: bool,
    /// Host do túnel SSH, quando a conexão passa por um
    pub ssh_host: Option<String>,
    pub group: Option<String>,
    pub secret: Secret,
}

impl RawConnection {
    /// Conexão vazia de um banco — os importadores preenchem o resto.
    pub fn new(name: impl Into<String>, driver: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            driver: driver.into(),
            host: String::new(),
            port: None,
            username: String::new(),
            database: None,
            uri: None,
            ssl_mode: None,
            client_certificate: false,
            ssh_host: None,
            group: None,
            secret: Secret::None,
        }
    }
}

/// Nome canônico do banco a partir do nome que cada app usa. Desconhecido
/// volta como veio — vira "sem suporte" com o nome que o usuário reconhece.
pub fn canonical_driver(raw: &str) -> String {
    let lower = raw.trim().to_ascii_lowercase();
    let name = match lower.as_str() {
        "postgresql" | "postgres" | "pgsql" | "pg" => "PostgreSQL",
        "mongodb" | "mongo" | "mongodb+srv" => "MongoDB",
        "redis" | "rediss" => "Redis",
        "sqlite" | "sqlite3" => "SQLite",
        "mysql" => "MySQL",
        "mariadb" => "MariaDB",
        "sqlserver" | "mssql" => "SQL Server",
        "oracle" => "Oracle",
        "redshift" => "Redshift",
        "cockroachdb" | "cockroach" => "CockroachDB",
        "clickhouse" => "ClickHouse",
        "cassandra" => "Cassandra",
        _ => return raw.trim().to_string(),
    };
    name.to_string()
}

pub fn db_type_of(driver: &str) -> Option<DatabaseType> {
    match driver {
        "PostgreSQL" => Some(DatabaseType::Postgres),
        "MongoDB" => Some(DatabaseType::Mongodb),
        "Redis" => Some(DatabaseType::Redis),
        "SQLite" => Some(DatabaseType::Sqlite),
        _ => None,
    }
}

/// Os campos do servidor que a conexão vira, e o que se perde no caminho.
struct Mapped {
    db_type: Option<DatabaseType>,
    host: String,
    port: u16,
    username: String,
    default_database: Option<String>,
    connection_uri: Option<String>,
    ssl_enabled: bool,
    warnings: Vec<ImportWarning>,
}

fn map(raw: &RawConnection) -> Mapped {
    let db_type = db_type_of(&raw.driver);
    let mut warnings = Vec::new();

    if let Some(host) = &raw.ssh_host {
        warnings.push(ImportWarning::SshTunnel { host: host.clone() });
    }
    if db_type.is_none() {
        warnings.push(ImportWarning::UnsupportedType {
            driver: raw.driver.clone(),
        });
    }

    let ssl_mode = raw.ssl_mode.as_deref().map(str::to_ascii_lowercase);
    // `prefer`/`allow` tentam SSL e aceitam sem: o Octapus não tem o meio-termo
    // e conecta sem, que é o que funciona com qualquer servidor
    let ssl_enabled = matches!(
        ssl_mode.as_deref(),
        Some("require" | "verify-ca" | "verify-full")
    );
    if let Some(mode @ ("verify-ca" | "verify-full")) = ssl_mode.as_deref() {
        warnings.push(ImportWarning::SslNotVerified {
            mode: mode.to_string(),
        });
    }
    if raw.client_certificate {
        warnings.push(ImportWarning::ClientCertificate);
    }

    let database = raw.database.clone().filter(|db| !db.trim().is_empty());

    // SQLite: o caminho do arquivo mora na URI, como no formulário
    if db_type == Some(DatabaseType::Sqlite) {
        let path = raw.uri.clone().or(database).unwrap_or_default();
        if !path.is_empty() && !Path::new(&expand_home(&path)).exists() {
            warnings.push(ImportWarning::FileMissing { path: path.clone() });
        }
        return Mapped {
            db_type,
            host: String::new(),
            port: 0,
            username: String::new(),
            default_database: Some("main".into()),
            connection_uri: Some(path).filter(|p| !p.is_empty()),
            ssl_enabled: false,
            warnings,
        };
    }

    let default_port = db_type.map(|t| t.default_port()).unwrap_or(0);
    Mapped {
        db_type,
        host: if raw.host.is_empty() && raw.uri.is_none() {
            "localhost".into()
        } else {
            raw.host.clone()
        },
        port: raw.port.filter(|p| *p != 0).unwrap_or(default_port),
        username: raw.username.clone(),
        default_database: database,
        connection_uri: raw.uri.clone(),
        ssl_enabled,
        warnings,
    }
}

/// O candidato da prévia. `existing` serve para apontar duplicados.
pub fn to_candidate(key: usize, raw: &RawConnection, existing: &[Server]) -> ImportCandidate {
    let mapped = map(raw);
    let mut warnings = mapped.warnings;
    let has_password = matches!(raw.secret, Secret::Plain(_));
    let in_keychain = matches!(raw.secret, Secret::Keychain { .. });
    // URI com credencial não precisa de senha à parte; SQLite não tem senha
    let needs_password = mapped.db_type.is_some()
        && mapped.db_type != Some(DatabaseType::Sqlite)
        && mapped.connection_uri.is_none()
        && !mapped.username.is_empty();
    if needs_password && !has_password && !in_keychain {
        warnings.push(ImportWarning::NoPassword);
    }

    let duplicate_of = mapped.db_type.and_then(|db_type| {
        existing
            .iter()
            .find(|server| {
                server.db_type == db_type
                    && match (&mapped.connection_uri, &server.connection_uri) {
                        (Some(a), Some(b)) => a.trim() == b.trim(),
                        (None, None) => {
                            server.host.eq_ignore_ascii_case(&mapped.host)
                                && server.port == mapped.port
                                && server.username == mapped.username
                                && server.default_database == mapped.default_database
                        }
                        _ => false,
                    }
            })
            .and_then(|server| server.id)
    });

    ImportCandidate {
        key,
        name: raw.name.clone(),
        db_type: mapped.db_type,
        driver: raw.driver.clone(),
        host: mapped.host,
        port: mapped.port,
        username: mapped.username,
        default_database: mapped.default_database,
        // A URI pode trazer a senha: a prévia vai para o front, a senha não
        connection_uri: mapped.connection_uri.as_deref().map(redact_uri),
        ssl_enabled: mapped.ssl_enabled,
        group: raw.group.clone(),
        has_password,
        password_in_keychain: in_keychain,
        duplicate_of,
        warnings,
    }
}

/// O servidor a criar; `None` quando o Octapus não abre esse banco.
pub fn to_server_input(
    raw: &RawConnection,
    password: String,
    group_id: Option<i64>,
) -> Option<ServerInput> {
    let mapped = map(raw);
    Some(ServerInput {
        name: raw.name.clone(),
        db_type: mapped.db_type?,
        host: mapped.host,
        port: mapped.port,
        username: mapped.username,
        password,
        default_database: mapped.default_database,
        ssl_enabled: Some(mapped.ssl_enabled),
        connection_uri: mapped.connection_uri,
        scope_databases: None,
        scope_schemas: None,
        group_id,
    })
}

/// `scheme://user:senha@host` → `scheme://user:•••@host`.
fn redact_uri(uri: &str) -> String {
    let Some((scheme, rest)) = uri.split_once("://") else {
        return uri.to_string();
    };
    let authority_end = rest.find('/').unwrap_or(rest.len());
    match rest[..authority_end].rfind('@') {
        Some(at) => match rest[..at].split_once(':') {
            Some((user, _)) => format!("{scheme}://{user}:•••{}", &rest[at..]),
            None => uri.to_string(),
        },
        None => uri.to_string(),
    }
}

/// `~/dados/app.db` → caminho absoluto (para conferir se o arquivo existe).
fn expand_home(path: &str) -> String {
    match (path.strip_prefix("~/"), dirs::home_dir()) {
        (Some(rest), Some(home)) => home.join(rest).to_string_lossy().into_owned(),
        _ => path.to_string(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// .pgpass
// ─────────────────────────────────────────────────────────────────────────────

/// Uma linha do `.pgpass`: `host:port:database:username:password`, com `*`
/// valendo qualquer coisa e `\:`/`\\` escapados.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PgPassEntry {
    fields: [String; 4],
    password: String,
}

pub fn parse_pgpass(text: &str) -> Vec<PgPassEntry> {
    text.lines()
        .filter(|line| !line.trim_start().starts_with('#') && !line.trim().is_empty())
        .filter_map(|line| {
            let mut parts = Vec::with_capacity(5);
            let mut current = String::new();
            let mut chars = line.chars();
            while let Some(c) = chars.next() {
                match c {
                    '\\' => current.extend(chars.next()),
                    ':' if parts.len() < 4 => parts.push(std::mem::take(&mut current)),
                    _ => current.push(c),
                }
            }
            parts.push(current);
            let [host, port, database, username, password]: [String; 5] = parts.try_into().ok()?;
            Some(PgPassEntry {
                fields: [host, port, database, username],
                password,
            })
        })
        .collect()
}

/// A senha da primeira linha que casa, como o libpq faz.
pub fn pgpass_lookup<'a>(
    entries: &'a [PgPassEntry],
    host: &str,
    port: u16,
    database: &str,
    username: &str,
) -> Option<&'a str> {
    let wanted = [
        host.to_string(),
        port.to_string(),
        database.to_string(),
        username.to_string(),
    ];
    entries
        .iter()
        .find(|entry| {
            entry
                .fields
                .iter()
                .zip(&wanted)
                .all(|(field, value)| field == "*" || field == value)
        })
        .map(|entry| entry.password.as_str())
}

/// Completa com o `.pgpass` a senha das conexões Postgres que vieram sem.
pub fn fill_from_pgpass(raws: &mut [RawConnection], entries: &[PgPassEntry]) {
    if entries.is_empty() {
        return;
    }
    for raw in raws.iter_mut() {
        if raw.secret != Secret::None || raw.driver != "PostgreSQL" || raw.uri.is_some() {
            continue;
        }
        let host = if raw.host.is_empty() {
            "localhost"
        } else {
            &raw.host
        };
        let port = raw.port.unwrap_or(5432);
        let database = raw.database.as_deref().unwrap_or("postgres");
        if let Some(password) = pgpass_lookup(entries, host, port, database, &raw.username) {
            raw.secret = Secret::Plain(password.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pg(host: &str) -> RawConnection {
        RawConnection {
            host: host.into(),
            port: Some(5432),
            username: "app".into(),
            database: Some("loja".into()),
            ..RawConnection::new("Loja", "PostgreSQL")
        }
    }

    #[test]
    fn warnings_cover_what_octapus_cannot_do() {
        let mut raw = pg("db.interno");
        raw.ssh_host = Some("bastion.exemplo.com".into());
        raw.ssl_mode = Some("verify-full".into());
        raw.client_certificate = true;
        let candidate = to_candidate(0, &raw, &[]);
        assert!(candidate.ssl_enabled);
        assert_eq!(
            candidate.warnings,
            vec![
                ImportWarning::SshTunnel {
                    host: "bastion.exemplo.com".into()
                },
                ImportWarning::SslNotVerified {
                    mode: "verify-full".into()
                },
                ImportWarning::ClientCertificate,
                ImportWarning::NoPassword,
            ]
        );

        let mysql = RawConnection::new("Legado", "MySQL");
        let candidate = to_candidate(1, &mysql, &[]);
        assert_eq!(candidate.db_type, None);
        assert!(candidate
            .warnings
            .contains(&ImportWarning::UnsupportedType {
                driver: "MySQL".into()
            }));
        assert!(to_server_input(&mysql, String::new(), None).is_none());
    }

    #[test]
    fn prefer_connects_without_ssl_and_password_is_kept() {
        let mut raw = pg("localhost");
        raw.ssl_mode = Some("prefer".into());
        raw.secret = Secret::Plain("s3cret".into());
        let candidate = to_candidate(0, &raw, &[]);
        assert!(!candidate.ssl_enabled);
        assert!(candidate.has_password);
        assert!(candidate.warnings.is_empty());

        let input = to_server_input(&raw, "s3cret".into(), Some(7)).unwrap();
        assert_eq!(input.password, "s3cret");
        assert_eq!(input.group_id, Some(7));
        assert_eq!(input.default_database.as_deref(), Some("loja"));
    }

    #[test]
    fn sqlite_path_goes_to_uri_and_missing_file_warns() {
        let raw = RawConnection {
            database: Some("/nao/existe/app.db".into()),
            ..RawConnection::new("Notas", "SQLite")
        };
        let candidate = to_candidate(0, &raw, &[]);
        assert_eq!(
            candidate.connection_uri.as_deref(),
            Some("/nao/existe/app.db")
        );
        assert_eq!(candidate.host, "");
        assert_eq!(
            candidate.warnings,
            vec![ImportWarning::FileMissing {
                path: "/nao/existe/app.db".into()
            }]
        );
    }

    #[test]
    fn duplicates_match_host_port_user_and_database() {
        let existing = Server {
            id: Some(42),
            name: "Já tenho".into(),
            db_type: DatabaseType::Postgres,
            host: "LOCALHOST".into(),
            port: 5432,
            username: "app".into(),
            password: String::new(),
            default_database: Some("loja".into()),
            ssl_enabled: false,
            connection_uri: None,
            created_at: 0,
            scope_databases: None,
            scope_schemas: None,
            group_id: None,
        };
        assert_eq!(
            to_candidate(0, &pg("localhost"), &[existing.clone()]).duplicate_of,
            Some(42)
        );
        assert_eq!(
            to_candidate(0, &pg("outro"), &[existing]).duplicate_of,
            None
        );
    }

    #[test]
    fn pgpass_parsing_escapes_and_wildcards() {
        let entries = parse_pgpass(
            "# comentário\n\
             db.exemplo.com:5432:*:app:se\\:nha\n\
             *:*:*:postgres:geral\n",
        );
        assert_eq!(entries.len(), 2);
        assert_eq!(
            pgpass_lookup(&entries, "db.exemplo.com", 5432, "x", "app"),
            Some("se:nha")
        );
        assert_eq!(
            pgpass_lookup(&entries, "outro", 6543, "y", "postgres"),
            Some("geral")
        );
        assert_eq!(pgpass_lookup(&entries, "outro", 5432, "x", "app"), None);

        let mut raws = vec![
            RawConnection {
                username: "postgres".into(),
                ..pg("h")
            },
            RawConnection {
                secret: Secret::Plain("mantém".into()),
                ..pg("h")
            },
        ];
        fill_from_pgpass(&mut raws, &entries);
        assert_eq!(raws[0].secret, Secret::Plain("geral".into()));
        assert_eq!(raws[1].secret, Secret::Plain("mantém".into()));
    }

    #[test]
    fn preview_never_carries_the_uri_password() {
        let raw = RawConnection {
            uri: Some("mongodb+srv://app:s3cr3t@cluster0.x.net/db".into()),
            ..RawConnection::new("Atlas", "MongoDB")
        };
        let candidate = to_candidate(0, &raw, &[]);
        assert_eq!(
            candidate.connection_uri.as_deref(),
            Some("mongodb+srv://app:•••@cluster0.x.net/db")
        );
        // O servidor criado leva a URI inteira
        let input = to_server_input(&raw, String::new(), None).unwrap();
        assert_eq!(input.connection_uri, raw.uri);
        assert_eq!(redact_uri("redis://cache:6379"), "redis://cache:6379");
        assert_eq!(redact_uri("redis://:pw@cache"), "redis://:•••@cache");
    }

    #[test]
    fn driver_names_are_normalized() {
        assert_eq!(canonical_driver("postgres"), "PostgreSQL");
        assert_eq!(canonical_driver("Mongo"), "MongoDB");
        assert_eq!(canonical_driver("MSSQL"), "SQL Server");
        assert_eq!(canonical_driver("Snowflake"), "Snowflake");
    }
}
