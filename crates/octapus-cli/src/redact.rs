//! Nenhuma saída do `octapus` leva senha: a URI de conexão sai sem a
//! credencial, onde quer que ela esteja (no `user:senha@` ou num parâmetro).

/// Parâmetros de query que carregam segredo.
const SECRET_PARAMS: &[&str] = &["password", "pass", "pwd", "sslpassword", "authtoken", "token"];

/// `scheme://user:senha@host/db?password=x` → `scheme://user:***@host/db?password=***`.
pub fn uri(uri: &str) -> String {
    let Some((scheme, rest)) = uri.split_once("://") else {
        return uri.to_string();
    };
    let (before_query, query) = match rest.split_once('?') {
        Some((before, query)) => (before, Some(query)),
        None => (rest, None),
    };

    let authority_end = before_query.find('/').unwrap_or(before_query.len());
    let (authority, path) = before_query.split_at(authority_end);
    let authority = match authority.rfind('@') {
        Some(at) => {
            let (userinfo, host) = authority.split_at(at);
            match userinfo.split_once(':') {
                Some((user, _)) => format!("{user}:***{host}"),
                None => authority.to_string(),
            }
        }
        None => authority.to_string(),
    };

    let mut out = format!("{scheme}://{authority}{path}");
    if let Some(query) = query {
        let params: Vec<String> = query
            .split('&')
            .map(|param| match param.split_once('=') {
                Some((name, _)) if SECRET_PARAMS.contains(&name.to_ascii_lowercase().as_str()) => {
                    format!("{name}=***")
                }
                _ => param.to_string(),
            })
            .collect();
        out.push('?');
        out.push_str(&params.join("&"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::uri;

    #[test]
    fn hides_the_password_wherever_it_is() {
        assert_eq!(uri("postgres://ana:s3cret@db.example.com:5432/loja"), "postgres://ana:***@db.example.com:5432/loja");
        assert_eq!(
            uri("mongodb+srv://ana:p%40ss@cluster.example.net/?retryWrites=true&authToken=abc"),
            "mongodb+srv://ana:***@cluster.example.net/?retryWrites=true&authToken=***"
        );
        // Senha com `@` e `:` dentro
        assert_eq!(uri("redis://:a:b@c@host:6379/0"), "redis://:***@host:6379/0");
        assert_eq!(uri("postgres://h/db?sslmode=require&password=x"), "postgres://h/db?sslmode=require&password=***");
    }

    #[test]
    fn leaves_what_has_no_secret() {
        assert_eq!(uri("postgres://ana@host/db"), "postgres://ana@host/db");
        assert_eq!(uri("redis://host:6379"), "redis://host:6379");
        assert_eq!(uri("/Users/ana/dados/app.db"), "/Users/ana/dados/app.db");
    }
}
