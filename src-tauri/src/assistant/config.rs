//! Onde fica a API do assistente de consulta.
//!
//! Vale a variável `OCTAPUS_ASSISTANT_URL` no ambiente do app (dev, outra API
//! para teste) e, sem ela, o valor embutido no build — o `release.yml` passa o
//! `vars.OCTAPUS_ASSISTANT_URL` (ver `build.rs`). Sem nenhuma das duas o
//! assistente fica desligado (`ASSISTANT_NOT_CONFIGURED`) e o resto do app
//! segue igual.

use std::net::IpAddr;

use reqwest::Url;

pub const URL_ENV: &str = "OCTAPUS_ASSISTANT_URL";

/// O valor do ambiente na hora do build (`None` se a variável não existia).
const BUILD_URL: Option<&str> = option_env!("OCTAPUS_ASSISTANT_URL");

/// A URL base da API, já validada e sem `/` no fim; `None` = não configurado.
pub fn base_url() -> Option<String> {
    let runtime = std::env::var(URL_ENV).ok();
    match resolve(runtime.as_deref(), BUILD_URL) {
        Ok(url) => url,
        Err(error) => {
            log::warn!("assistente: {URL_ENV} inválida, assistente desligado ({error})");
            None
        }
    }
}

/// O valor do ambiente vence o do build; valor vazio conta como ausente.
/// Um valor inválido não cai para o outro: desliga o assistente, com aviso no
/// log, em vez de falar em silêncio com outra API.
fn resolve(runtime: Option<&str>, build: Option<&str>) -> Result<Option<String>, String> {
    [runtime, build]
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|value| !value.is_empty())
        .map(normalize)
        .transpose()
}

/// `https` em qualquer host; `http` só na própria máquina (a API rodando em
/// dev): o token do participante vai no cabeçalho de cada pedido.
fn normalize(raw: &str) -> Result<String, String> {
    let url = Url::parse(raw).map_err(|e| format!("URL inválida: {e}"))?;

    let local = match url.host_str() {
        Some("localhost") => true,
        Some(host) => host
            .trim_start_matches('[')
            .trim_end_matches(']')
            .parse::<IpAddr>()
            .is_ok_and(|ip| ip.is_loopback()),
        None => false,
    };
    match url.scheme() {
        "https" => {}
        "http" if local => {}
        "http" => return Err("http só é aceito para localhost; use https".into()),
        scheme => return Err(format!("esquema {scheme} não suportado")),
    }
    if url.host_str().is_none() {
        return Err("URL sem host".into());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("a URL não pode levar usuário ou senha".into());
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err("a URL não pode levar query nem fragmento".into());
    }

    Ok(url.as_str().trim_end_matches('/').to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_value_wins_over_build_value() {
        assert_eq!(
            resolve(Some("https://dev.example.com"), Some("https://api.example.com")),
            Ok(Some("https://dev.example.com".into()))
        );
    }

    #[test]
    fn falls_back_to_build_value() {
        let expected = Ok(Some("https://api.example.com".into()));
        assert_eq!(resolve(None, Some("https://api.example.com")), expected);
        // Vazio ou só espaços conta como ausente
        assert_eq!(resolve(Some(""), Some("https://api.example.com")), expected);
        assert_eq!(resolve(Some("  "), Some("https://api.example.com")), expected);
    }

    #[test]
    fn nothing_configured_is_none() {
        assert_eq!(resolve(None, None), Ok(None));
        assert_eq!(resolve(Some(""), Some(" ")), Ok(None));
    }

    #[test]
    fn invalid_runtime_value_does_not_fall_back() {
        assert!(resolve(Some("not a url"), Some("https://api.example.com")).is_err());
    }

    #[test]
    fn normalizes_trailing_slash_and_keeps_path_prefix() {
        assert_eq!(normalize("https://api.example.com/"), Ok("https://api.example.com".into()));
        assert_eq!(
            normalize("https://example.com/assistant/"),
            Ok("https://example.com/assistant".into())
        );
    }

    #[test]
    fn http_only_on_loopback() {
        assert!(normalize("http://localhost:8080").is_ok());
        assert!(normalize("http://127.0.0.1:8080").is_ok());
        assert!(normalize("http://[::1]:8080").is_ok());
        assert!(normalize("http://api.example.com").is_err());
    }

    #[test]
    fn rejects_other_schemes_credentials_and_query() {
        assert!(normalize("ftp://api.example.com").is_err());
        assert!(normalize("https://user:pass@api.example.com").is_err());
        assert!(normalize("https://api.example.com/?token=x").is_err());
        assert!(normalize("https://api.example.com/#x").is_err());
    }
}
