//! URLs de conexão: as coladas pelo usuário (`postgres://…`, `mongodb+srv://…`,
//! `redis://…`, caminho de SQLite) e as JDBC do DataGrip. Parser próprio e
//! pequeno: o crate `url` recusa a lista de hosts do Mongo (`h1:27017,h2`).

use super::mapping::{canonical_driver, RawConnection, Secret};

/// Partes de `scheme://[user[:senha]@]hosts[/path][?query]`.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct UrlParts {
    pub scheme: String,
    pub user: Option<String>,
    pub password: Option<String>,
    /// Hosts como vieram (`h1:27017,h2:27017`)
    pub hosts: String,
    pub path: Option<String>,
    pub query: Vec<(String, String)>,
}

impl UrlParts {
    /// Primeiro host e porta (`[::1]:5432` incluso).
    pub fn first_host(&self) -> (String, Option<u16>) {
        let first = self.hosts.split(',').next().unwrap_or_default();
        if let Some(rest) = first.strip_prefix('[') {
            let (host, after) = rest.split_once(']').unwrap_or((rest, ""));
            return (
                host.to_string(),
                after.strip_prefix(':').and_then(|p| p.parse().ok()),
            );
        }
        match first.rsplit_once(':') {
            Some((host, port)) if port.chars().all(|c| c.is_ascii_digit()) => {
                (host.to_string(), port.parse().ok())
            }
            _ => (first.to_string(), None),
        }
    }

    pub fn param(&self, name: &str) -> Option<&str> {
        self.query
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

pub fn parse_url(input: &str) -> Option<UrlParts> {
    let (scheme, rest) = input.trim().split_once("://")?;
    let (rest, query) = match rest.split_once('?') {
        Some((rest, query)) => (rest, query),
        None => (rest, ""),
    };
    let (authority, path) = match rest.split_once('/') {
        Some((authority, path)) => (authority, Some(path)),
        None => (rest, None),
    };
    // A senha pode ter `@` (sem escape): o último `@` separa a credencial
    let (userinfo, hosts) = match authority.rsplit_once('@') {
        Some((userinfo, hosts)) => (Some(userinfo), hosts),
        None => (None, authority),
    };
    let (user, password) = match userinfo {
        Some(info) => match info.split_once(':') {
            Some((user, password)) => (Some(decode(user)), Some(decode(password))),
            None => (Some(decode(info)), None),
        },
        None => (None, None),
    };
    let query = query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| match pair.split_once('=') {
            Some((key, value)) => (decode(key), decode(value)),
            None => (decode(pair), String::new()),
        })
        .collect();

    Some(UrlParts {
        scheme: scheme.to_ascii_lowercase(),
        user: user.filter(|u| !u.is_empty()),
        password,
        hosts: hosts.to_string(),
        path: path.map(decode).filter(|p| !p.is_empty()),
        query,
    })
}

/// Decodifica `%XX`; sequência inválida fica como está.
fn decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            // Pelos bytes, não pela `str`: `%` seguido de acento não é fronteira de char
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(byte) = hex.and_then(|hex| u8::from_str_radix(hex, 16).ok()) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Uma URL colada → conexão. Linhas que não são URL nem caminho são ignoradas.
///
/// Postgres e o caso simples de Redis/Mongo viram campos (a senha vai para o
/// cofre); o que só a URI descreve (SRV, vários hosts, parâmetros) fica como
/// URI, com a credencial dentro — igual a quem cola a URI no formulário.
pub fn connection_from_url(line: &str) -> Option<RawConnection> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return None;
    }

    // Caminho de arquivo SQLite (ou `sqlite:///caminho`)
    let sqlite_path = line
        .strip_prefix("sqlite://")
        .or_else(|| line.strip_prefix("sqlite:"))
        .or_else(|| (line.starts_with('/') || line.starts_with("~/")).then_some(line));
    if let Some(path) = sqlite_path {
        let name = std::path::Path::new(path)
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string());
        return Some(RawConnection {
            database: Some(path.to_string()),
            ..RawConnection::new(name, "SQLite")
        });
    }

    let parts = parse_url(line)?;
    let driver = canonical_driver(&parts.scheme);
    let (host, port) = parts.first_host();
    let mut raw = RawConnection {
        host: host.clone(),
        port,
        username: parts.user.clone().unwrap_or_default(),
        database: parts.path.clone(),
        ..RawConnection::new(host_label(&host, parts.path.as_deref()), driver.clone())
    };
    if let Some(password) = &parts.password {
        raw.secret = Secret::Plain(password.clone());
    }

    match driver.as_str() {
        "PostgreSQL" => {
            raw.ssl_mode = parts.param("sslmode").map(str::to_string);
            raw.client_certificate = parts.param("sslcert").is_some();
        }
        "MongoDB" => {
            let complex = parts.scheme == "mongodb+srv"
                || parts.hosts.contains(',')
                || !parts.query.is_empty();
            if complex {
                raw.uri = Some(line.to_string());
            }
            // `/db` na URI do Mongo é o banco de autenticação/padrão
        }
        "Redis" if parts.scheme == "rediss" || !parts.query.is_empty() => {
            raw.uri = Some(line.to_string());
        }
        _ => {}
    }
    Some(raw)
}

/// "loja @ db.exemplo.com" — o nome sugerido de uma conexão vinda de URL.
fn host_label(host: &str, database: Option<&str>) -> String {
    match database.filter(|db| !db.is_empty()) {
        Some(db) => format!("{db} @ {host}"),
        None => host.to_string(),
    }
}

/// URL JDBC (DataGrip) → banco, host, porta e database.
/// `jdbc:postgresql://h:5432/db?…`, `jdbc:sqlite:/caminho`, `jdbc:redis://h/0`.
pub fn parse_jdbc(url: &str) -> Option<(String, RawConnection)> {
    let rest = url.trim().strip_prefix("jdbc:").unwrap_or(url.trim());
    let (subprotocol, tail) = rest.split_once(':')?;
    let driver = canonical_driver(subprotocol);

    if driver == "SQLite" {
        let path = tail.strip_prefix("//").unwrap_or(tail);
        return Some((
            driver.clone(),
            RawConnection {
                database: Some(path.to_string()),
                ..RawConnection::new("", driver)
            },
        ));
    }

    let parts = parse_url(&format!("{subprotocol}:{tail}"))?;
    let (host, port) = parts.first_host();
    let database = parts
        .path
        .as_deref()
        .map(|p| p.split(';').next().unwrap_or(p).to_string());
    let mut raw = RawConnection {
        host,
        port,
        username: parts.user.clone().unwrap_or_default(),
        database,
        ..RawConnection::new("", driver.clone())
    };
    raw.ssl_mode = parts.param("sslmode").map(str::to_string);
    if driver == "MongoDB" && (parts.scheme == "mongodb+srv" || parts.hosts.contains(',')) {
        raw.uri = Some(format!("{subprotocol}:{tail}"));
    }
    Some((driver, raw))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn postgres_url_becomes_fields_with_password() {
        let raw =
            connection_from_url("postgresql://app:p%40ss@db.exemplo.com:6543/loja?sslmode=require")
                .unwrap();
        assert_eq!(raw.driver, "PostgreSQL");
        assert_eq!(raw.name, "loja @ db.exemplo.com");
        assert_eq!(
            (raw.host.as_str(), raw.port),
            ("db.exemplo.com", Some(6543))
        );
        assert_eq!(raw.username, "app");
        assert_eq!(raw.database.as_deref(), Some("loja"));
        assert_eq!(raw.ssl_mode.as_deref(), Some("require"));
        assert_eq!(raw.secret, Secret::Plain("p@ss".into()));
        assert_eq!(raw.uri, None);
    }

    #[test]
    fn mongo_srv_and_replica_set_keep_the_uri() {
        let srv = "mongodb+srv://app:x@cluster0.exemplo.mongodb.net/catalogo";
        let raw = connection_from_url(srv).unwrap();
        assert_eq!(raw.driver, "MongoDB");
        assert_eq!(raw.uri.as_deref(), Some(srv));

        let rs = connection_from_url("mongodb://h1:27017,h2:27018/app").unwrap();
        assert_eq!((rs.host.as_str(), rs.port), ("h1", Some(27017)));
        assert!(rs.uri.is_some());

        let simple = connection_from_url("mongodb://localhost:27017").unwrap();
        assert_eq!(simple.uri, None);
    }

    #[test]
    fn redis_sqlite_and_garbage() {
        let redis = connection_from_url("redis://:segredo@cache.local:6380/2").unwrap();
        assert_eq!(redis.driver, "Redis");
        assert_eq!(redis.username, "");
        assert_eq!(redis.secret, Secret::Plain("segredo".into()));
        assert_eq!(redis.database.as_deref(), Some("2"));
        assert!(connection_from_url("rediss://cache.local")
            .unwrap()
            .uri
            .is_some());

        let sqlite = connection_from_url("~/dados/notas.db").unwrap();
        assert_eq!(
            (sqlite.driver.as_str(), sqlite.name.as_str()),
            ("SQLite", "notas")
        );
        assert_eq!(sqlite.database.as_deref(), Some("~/dados/notas.db"));

        assert!(connection_from_url("isso não é url").is_none());
        assert!(connection_from_url("# comentário").is_none());
        let mysql = connection_from_url("mysql://root@localhost/app").unwrap();
        assert_eq!(mysql.driver, "MySQL");
    }

    #[test]
    fn ipv6_and_bad_escapes() {
        let parts = parse_url("postgres://u@[::1]:5433/db").unwrap();
        assert_eq!(parts.first_host(), ("::1".to_string(), Some(5433)));
        assert_eq!(decode("100%"), "100%");
        assert_eq!(decode("a%2Fb%zz"), "a/b%zz");
        assert_eq!(decode("%é1"), "%é1");
    }

    #[test]
    fn jdbc_urls() {
        let (driver, raw) =
            parse_jdbc("jdbc:postgresql://localhost:5432/loja?sslmode=verify-ca").unwrap();
        assert_eq!(driver, "PostgreSQL");
        assert_eq!((raw.host.as_str(), raw.port), ("localhost", Some(5432)));
        assert_eq!(raw.database.as_deref(), Some("loja"));
        assert_eq!(raw.ssl_mode.as_deref(), Some("verify-ca"));

        let (driver, raw) = parse_jdbc("jdbc:sqlite:/Users/voce/app.db").unwrap();
        assert_eq!(driver, "SQLite");
        assert_eq!(raw.database.as_deref(), Some("/Users/voce/app.db"));

        let (driver, _) = parse_jdbc("jdbc:mysql://localhost:3306/app").unwrap();
        assert_eq!(driver, "MySQL");
        let (driver, raw) = parse_jdbc("jdbc:redis://localhost:6379/0").unwrap();
        assert_eq!(
            (driver.as_str(), raw.database.as_deref()),
            ("Redis", Some("0"))
        );
    }
}
