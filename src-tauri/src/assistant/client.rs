//! Cliente HTTP da API do assistente (octapus-assistant-api, contrato 1.4.0).
//!
//! Só fala o protocolo: Bearer com o token do participante, JSON em camelCase
//! e a conversão das respostas de erro (`error.rs`). Onde o token mora e o que
//! fazer com cada erro é dos comandos (`commands/assistant.rs`).

use std::time::Duration;

use chrono::Utc;
use reqwest::header::RETRY_AFTER;
use reqwest::{Method, RequestBuilder, Response, StatusCode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::error::{self, AssistantCode, AssistantError};
use crate::error::{error_chain, Result};
use crate::models::{AssistantConsentTerm, AssistantMe, AssistantModel, AssistantParticipant};

/// Pedidos comuns (JSON pequeno). O stream de tentativas (SSE) vai precisar de
/// outro limite: este derrubaria a geração no meio.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// O health check entra no status da tela: responde rápido ou é "fora do ar".
const HEALTH_TIMEOUT: Duration = Duration::from_secs(5);

pub struct ApiClient {
    http: reqwest::Client,
    /// URL base validada, sem `/` no fim (`config.rs`)
    base: String,
}

/// `GET /v1/health`; o 503 traz o mesmo corpo, com o banco da API fora.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Health {
    /// `ok` ou `degraded`
    pub status: String,
    pub min_app_version: Option<String>,
}

/// `POST /v1/auth/redeem`
#[derive(Deserialize)]
pub struct Redeemed {
    pub token: String,
    pub participant: AssistantParticipant,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RedeemRequest<'a> {
    invite_code: &'a str,
    device_label: Option<&'a str>,
}

#[derive(Serialize)]
struct ConsentRequest<'a> {
    version: &'a str,
}

#[derive(Deserialize)]
struct ModelList {
    models: Vec<AssistantModel>,
}

impl ApiClient {
    pub fn new(base: &str) -> Result<Self> {
        // O reqwest vem sem provedor de criptografia (como no plugin do
        // updater, que instala o mesmo `ring` na primeira checagem)
        if rustls::crypto::CryptoProvider::get_default().is_none() {
            let _ = rustls::crypto::ring::default_provider().install_default();
        }

        let http = reqwest::Client::builder()
            .user_agent(concat!("octapus-db/", env!("CARGO_PKG_VERSION")))
            .timeout(REQUEST_TIMEOUT)
            .connect_timeout(CONNECT_TIMEOUT)
            .build()
            .map_err(transport)?;

        Ok(Self {
            http,
            base: base.to_string(),
        })
    }

    pub async fn health(&self) -> Result<Health> {
        let response = self
            .request(Method::GET, "/v1/health", None)
            .timeout(HEALTH_TIMEOUT)
            .send()
            .await
            .map_err(transport)?;
        if response.status() == StatusCode::SERVICE_UNAVAILABLE {
            return decode(response).await;
        }
        decode(check(response).await?).await
    }

    pub async fn redeem(&self, invite_code: &str, device_label: Option<&str>) -> Result<Redeemed> {
        let body = RedeemRequest {
            invite_code,
            device_label,
        };
        self.json(self.request(Method::POST, "/v1/auth/redeem", None).json(&body))
            .await
    }

    pub async fn me(&self, token: &str) -> Result<AssistantMe> {
        self.json(self.request(Method::GET, "/v1/me", Some(token))).await
    }

    pub async fn consent_current(&self, token: &str) -> Result<AssistantConsentTerm> {
        self.json(self.request(Method::GET, "/v1/consents/current", Some(token)))
            .await
    }

    pub async fn consent_accept(&self, token: &str, version: &str) -> Result<()> {
        let request = self
            .request(Method::POST, "/v1/consents", Some(token))
            .json(&ConsentRequest { version });
        send(request).await.map(drop)
    }

    pub async fn models(&self, token: &str) -> Result<Vec<AssistantModel>> {
        let list: ModelList = self
            .json(self.request(Method::GET, "/v1/models", Some(token)))
            .await?;
        Ok(list.models)
    }

    fn request(&self, method: Method, path: &str, token: Option<&str>) -> RequestBuilder {
        let request = self.http.request(method, format!("{}{path}", self.base));
        match token {
            Some(token) => request.bearer_auth(token),
            None => request,
        }
    }

    async fn json<T: DeserializeOwned>(&self, request: RequestBuilder) -> Result<T> {
        decode(send(request).await?).await
    }
}

async fn send(request: RequestBuilder) -> Result<Response> {
    check(request.send().await.map_err(transport)?).await
}

/// Resposta de falha → `AssistantError` pelo corpo do contrato (ou pelo status).
async fn check(response: Response) -> Result<Response> {
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    let retry_after = response
        .headers()
        .get(RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    // Corpo ilegível conta como vazio: o status ainda decide
    let body = response.text().await.unwrap_or_default();
    Err(error::from_response(status.as_u16(), retry_after.as_deref(), &body, Utc::now()).into())
}

/// Corpo de sucesso fora do contrato: API de outra versão, proxy no caminho.
async fn decode<T: DeserializeOwned>(response: Response) -> Result<T> {
    response.json().await.map_err(|e| {
        AssistantError::new(
            AssistantCode::Internal,
            format!("resposta inesperada da API: {}", error_chain(&e.without_url())),
        )
        .into()
    })
}

/// Sem resposta HTTP (rede, DNS, TLS, tempo esgotado): a API está fora (C-19).
fn transport(e: reqwest::Error) -> crate::error::Error {
    AssistantError::new(AssistantCode::Unavailable, error_chain(&e.without_url())).into()
}

#[cfg(test)]
mod tests {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;
    use tokio::sync::oneshot;

    use super::*;
    use crate::error::Error;

    /// Servidor HTTP de uma resposta só: devolve `response` e entrega o pedido
    /// recebido (linha e cabeçalhos) para o teste conferir.
    async fn serve_once(response: &'static str) -> (String, oneshot::Receiver<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let (sent, received) = oneshot::channel();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            let mut buffer = [0u8; 4096];
            while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                let read = socket.read(&mut buffer).await.unwrap();
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..read]);
            }
            socket.write_all(response.as_bytes()).await.unwrap();
            let _ = sent.send(String::from_utf8_lossy(&request).into_owned());
        });
        (base, received)
    }

    fn http(status: &str, headers: &str, body: &str) -> &'static str {
        Box::leak(
            format!(
                "HTTP/1.1 {status}\r\ncontent-type: application/json\r\n{headers}content-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            )
            .into_boxed_str(),
        )
    }

    fn assistant_error(result: Result<impl std::fmt::Debug>) -> AssistantError {
        match result {
            Err(Error::Assistant(error)) => error,
            other => panic!("esperava erro do assistente, veio {other:?}"),
        }
    }

    #[tokio::test]
    async fn sends_bearer_and_decodes_me() {
        let body = r#"{"participant":{"id":"p1","label":"P-01"},"modelSelection":"free","consent":{"currentVersion":"v2","acceptedVersion":null},"quota":{"limitTokens":1000,"usedTokens":10,"reservedTokens":5}}"#;
        let (base, request) = serve_once(http("200 OK", "", body)).await;

        let me = ApiClient::new(&base).unwrap().me("tok_123").await.unwrap();
        assert_eq!(me.participant.label, "P-01");
        assert_eq!(me.consent.accepted_version, None);
        assert_eq!(me.quota.reserved_tokens, 5);

        let request = request.await.unwrap().to_lowercase();
        assert!(request.starts_with("get /v1/me "), "{request}");
        assert!(request.contains("authorization: bearer tok_123"), "{request}");
    }

    #[tokio::test]
    async fn error_body_and_retry_after_become_assistant_error() {
        let body = r#"{"error":{"code":"rate_limited","message":"slow down"}}"#;
        let (base, _) =
            serve_once(http("429 Too Many Requests", "retry-after: 12\r\n", body)).await;

        let error = assistant_error(ApiClient::new(&base).unwrap().models("tok").await);
        assert_eq!(error.code, AssistantCode::RateLimited);
        assert_eq!(error.message, "slow down");
        assert_eq!(error.retry_after, Some(Duration::from_secs(12)));
    }

    #[tokio::test]
    async fn degraded_health_still_decodes() {
        let body = r#"{"status":"degraded","version":"1.4.0","minAppVersion":null,"database":"unavailable"}"#;
        let (base, _) = serve_once(http("503 Service Unavailable", "", body)).await;

        let health = ApiClient::new(&base).unwrap().health().await.unwrap();
        assert_eq!(health.status, "degraded");
    }

    #[tokio::test]
    async fn unreachable_api_is_unavailable() {
        // Porta fechada: pega uma livre e solta antes do pedido
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        drop(listener);

        let error = assistant_error(ApiClient::new(&base).unwrap().me("tok").await);
        assert_eq!(error.code, AssistantCode::Unavailable);
    }
}
