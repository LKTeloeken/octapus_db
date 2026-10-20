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

use super::cancel::CancelFlag;
use super::error::{self, AssistantCode, AssistantError};
use super::language::Language;
use super::messages::AttemptRequest;
use super::sse::SseReader;
use crate::error::{error_chain, Error, Result};
use crate::models::{
    AssistantConsentTerm, AssistantConversation, AssistantConversationPage,
    AssistantConversationSummary, AssistantMe, AssistantModel, AssistantParticipant,
    AssistantTurnStatus, AssistantTurnSummary, AttemptOutcome,
};

/// Pedidos comuns (JSON pequeno). O stream de tentativas (SSE) vai precisar de
/// outro limite: este derrubaria a geração no meio.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
/// O health check entra no status da tela: responde rápido ou é "fora do ar".
const HEALTH_TIMEOUT: Duration = Duration::from_secs(5);
/// O stream de uma tentativa: acima do teto da API (3 min por tentativa)
const ATTEMPT_TIMEOUT: Duration = Duration::from_secs(5 * 60);
/// Sem nenhum byte por este tempo o stream morreu (a API manda keepalive a
/// cada 15 s)
const STREAM_IDLE: Duration = Duration::from_secs(45);

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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ConversationRequest<'a> {
    paradigm: &'a str,
    model_id: Option<&'a str>,
    title: Option<&'a str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TurnRequest<'a> {
    question: &'a str,
    language: Language,
    client_turn_id: &'a str,
}

#[derive(Serialize)]
struct FinishRequest {
    status: AssistantTurnStatus,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AttemptStarted {
    attempt_id: String,
    index: i64,
}

#[derive(Deserialize)]
struct Delta {
    text: String,
}

/// O progresso do stream, conforme chega.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttemptProgress {
    Started { attempt_id: String, index: i64 },
    Delta(String),
}

/// Como o stream de uma tentativa terminou.
#[derive(Debug, Clone, PartialEq)]
pub struct AttemptReply {
    pub attempt_id: String,
    pub index: i64,
    /// A resposta (`result`, já validada pela API) ou o erro dela (`error`:
    /// `invalid_output`, `model_refused`, `provider_error`…)
    pub result: std::result::Result<serde_json::Value, AssistantError>,
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

    pub async fn conversations(
        &self,
        token: &str,
        cursor: Option<&str>,
        limit: u32,
    ) -> Result<AssistantConversationPage> {
        // O cursor é opaco (vem da API): vai codificado na URL
        let mut url = reqwest::Url::parse(&format!("{}/v1/conversations", self.base)).map_err(|e| {
            Error::from(AssistantError::new(AssistantCode::Internal, e.to_string()))
        })?;
        url.query_pairs_mut().append_pair("limit", &limit.to_string());
        if let Some(cursor) = cursor {
            url.query_pairs_mut().append_pair("cursor", cursor);
        }
        self.json(self.http.get(url).bearer_auth(token)).await
    }

    pub async fn create_conversation(
        &self,
        token: &str,
        paradigm: &str,
        model_id: Option<&str>,
        title: Option<&str>,
    ) -> Result<AssistantConversationSummary> {
        let body = ConversationRequest {
            paradigm,
            model_id,
            title,
        };
        self.json(self.request(Method::POST, "/v1/conversations", Some(token)).json(&body))
            .await
    }

    pub async fn conversation(&self, token: &str, id: &str) -> Result<AssistantConversation> {
        let path = format!("/v1/conversations/{}", path_segment(id)?);
        self.json(self.request(Method::GET, &path, Some(token))).await
    }

    /// Abre o turno. O mesmo `client_turn_id` devolve o turno já criado (200
    /// em vez de 201): repetir depois de uma falha de rede é seguro.
    pub async fn create_turn(
        &self,
        token: &str,
        conversation_id: &str,
        question: &str,
        language: Language,
        client_turn_id: &str,
    ) -> Result<AssistantTurnSummary> {
        let path = format!("/v1/conversations/{}/turns", path_segment(conversation_id)?);
        let body = TurnRequest {
            question,
            language,
            client_turn_id,
        };
        self.json(self.request(Method::POST, &path, Some(token)).json(&body))
            .await
    }

    /// Gera uma tentativa e lê o stream até o `done`. Cancelar fecha a
    /// conexão: a API interrompe o provedor e devolve a reserva de cota.
    pub async fn create_attempt(
        &self,
        token: &str,
        turn_id: &str,
        request: &AttemptRequest,
        cancel: &CancelFlag,
        progress: &(dyn Fn(AttemptProgress) + Send + Sync),
    ) -> Result<AttemptReply> {
        let path = format!("/v1/turns/{}/attempts", path_segment(turn_id)?);
        let pending = self
            .request(Method::POST, &path, Some(token))
            .timeout(ATTEMPT_TIMEOUT)
            .json(request)
            .send();
        let response = tokio::select! {
            response = pending => check(response.map_err(transport)?).await?,
            _ = cancel.cancelled() => return Err(Error::Cancelled),
        };
        read_attempt_stream(response, cancel, progress).await
    }

    /// O que o app fez com a consulta da tentativa (repetir sobrescreve).
    pub async fn report_outcome(&self, token: &str, attempt_id: &str, outcome: &AttemptOutcome) -> Result<()> {
        let path = format!("/v1/attempts/{}/outcome", path_segment(attempt_id)?);
        send(self.request(Method::POST, &path, Some(token)).json(outcome))
            .await
            .map(drop)
    }

    pub async fn finish_turn(&self, token: &str, turn_id: &str, status: AssistantTurnStatus) -> Result<()> {
        let path = format!("/v1/turns/{}/finish", path_segment(turn_id)?);
        send(self.request(Method::POST, &path, Some(token)).json(&FinishRequest { status }))
            .await
            .map(drop)
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

/// Os ids da API são UUID: qualquer outra coisa no caminho da URL é recusada
/// antes de sair (nada de `../` vindo do front).
fn path_segment(id: &str) -> Result<&str> {
    let valid = id.len() == 36 && id.chars().all(|c| c.is_ascii_hexdigit() || c == '-');
    if valid {
        Ok(id)
    } else {
        Err(AssistantError::new(AssistantCode::InvalidRequest, "invalid id").into())
    }
}

/// Lê os eventos até o `done`: `attempt` (sempre o primeiro), `delta`,
/// `usage`, e `result` ou `error`. Stream que acaba antes, ou fica mudo além
/// do keepalive, é a API fora (`unavailable`).
async fn read_attempt_stream(
    mut response: Response,
    cancel: &CancelFlag,
    progress: &(dyn Fn(AttemptProgress) + Send + Sync),
) -> Result<AttemptReply> {
    let mut reader = SseReader::default();
    let mut started: Option<AttemptStarted> = None;
    let mut result: Option<std::result::Result<serde_json::Value, AssistantError>> = None;
    let unavailable = |message: &str| -> Error {
        AssistantError::new(AssistantCode::Unavailable, message).into()
    };

    loop {
        let chunk = tokio::select! {
            chunk = tokio::time::timeout(STREAM_IDLE, response.chunk()) => match chunk {
                Err(_) => return Err(unavailable("attempt stream went silent")),
                Ok(chunk) => chunk.map_err(transport)?,
            },
            _ = cancel.cancelled() => return Err(Error::Cancelled),
        };
        let Some(chunk) = chunk else {
            return Err(unavailable("attempt stream ended before done"));
        };

        for event in reader.feed(&chunk) {
            match event.event.as_str() {
                "attempt" => {
                    let attempt: AttemptStarted = serde_json::from_str(&event.data)
                        .map_err(|_| unavailable("malformed attempt event"))?;
                    progress(AttemptProgress::Started {
                        attempt_id: attempt.attempt_id.clone(),
                        index: attempt.index,
                    });
                    started = Some(attempt);
                }
                "delta" => {
                    if let Ok(delta) = serde_json::from_str::<Delta>(&event.data) {
                        progress(AttemptProgress::Delta(delta.text));
                    }
                }
                "result" => {
                    let value = serde_json::from_str(&event.data)
                        .map_err(|_| unavailable("malformed result event"))?;
                    result = Some(Ok(value));
                }
                "error" => {
                    result = Some(Err(error::from_response(200, None, &event.data, Utc::now())));
                }
                "done" => {
                    let started = started.ok_or_else(|| unavailable("stream without attempt event"))?;
                    let result = result.ok_or_else(|| unavailable("stream without result"))?;
                    return Ok(AttemptReply {
                        attempt_id: started.attempt_id,
                        index: started.index,
                        result,
                    });
                }
                _ => {} // usage e eventos novos: sem uso aqui
            }
        }
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

    /// Resposta SSE sem tamanho: o corpo vai até a conexão fechar.
    fn sse(events: &str) -> &'static str {
        Box::leak(
            format!(
                "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\nconnection: close\r\n\r\n{events}"
            )
            .into_boxed_str(),
        )
    }

    fn attempt_request() -> AttemptRequest {
        use crate::assistant::messages::{AttemptContext, Message, Role};
        AttemptRequest {
            prompt_version: "postgres-v1".into(),
            messages: vec![Message { role: Role::User, content: "x".into() }],
            context: AttemptContext {
                schema_truncated: false,
                schema_tokens_estimate: 1,
                previous_failure: None,
            },
        }
    }

    const TURN: &str = "6f1c2b8e-4d3a-4e5f-9a7b-1c2d3e4f5a6b";

    #[tokio::test]
    async fn attempt_stream_reads_progress_and_result() {
        let (base, request) = serve_once(sse(
            "event: attempt\ndata: {\"attemptId\":\"a-1\",\"index\":0}\n\n\
             : keepalive\n\n\
             event: delta\ndata: {\"text\":\"SEL\"}\n\n\
             event: usage\ndata: {\"inputTokens\":1}\n\n\
             event: result\ndata: {\"kind\":\"query\"}\n\n\
             event: done\ndata: {}\n\n",
        ))
        .await;
        let seen = std::sync::Mutex::new(Vec::new());
        let reply = ApiClient::new(&base)
            .unwrap()
            .create_attempt("tok", TURN, &attempt_request(), &CancelFlag::default(), &|p| seen.lock().unwrap().push(p))
            .await
            .unwrap();

        assert_eq!(reply.attempt_id, "a-1");
        assert_eq!(reply.result.unwrap(), serde_json::json!({ "kind": "query" }));
        assert_eq!(
            *seen.lock().unwrap(),
            [
                AttemptProgress::Started { attempt_id: "a-1".into(), index: 0 },
                AttemptProgress::Delta("SEL".into()),
            ]
        );
        let request = request.await.unwrap().to_lowercase();
        assert!(request.starts_with(&format!("post /v1/turns/{TURN}/attempts ")), "{request}");
    }

    #[tokio::test]
    async fn attempt_stream_error_event_and_early_end() {
        let (base, _) = serve_once(sse(
            "event: attempt\ndata: {\"attemptId\":\"a-1\",\"index\":2}\n\n\
             event: error\ndata: {\"error\":{\"code\":\"invalid_output\",\"message\":\"cut\"}}\n\n\
             event: done\ndata: {}\n\n",
        ))
        .await;
        let reply = ApiClient::new(&base)
            .unwrap()
            .create_attempt("tok", TURN, &attempt_request(), &CancelFlag::default(), &|_| {})
            .await
            .unwrap();
        assert_eq!(reply.index, 2);
        assert_eq!(reply.result.unwrap_err().code, AssistantCode::InvalidOutput);

        // A conexão cai antes do `done`: API fora
        let (base, _) = serve_once(sse("event: attempt\ndata: {\"attemptId\":\"a\",\"index\":0}\n\n")).await;
        let error = assistant_error(
            ApiClient::new(&base)
                .unwrap()
                .create_attempt("tok", TURN, &attempt_request(), &CancelFlag::default(), &|_| {})
                .await,
        );
        assert_eq!(error.code, AssistantCode::Unavailable);
    }

    #[tokio::test]
    async fn attempt_errors_before_the_stream_are_api_errors() {
        let body = r#"{"error":{"code":"turn_limit_exceeded","message":"limit"}}"#;
        let (base, _) = serve_once(http("409 Conflict", "", body)).await;
        let error = assistant_error(
            ApiClient::new(&base)
                .unwrap()
                .create_attempt("tok", TURN, &attempt_request(), &CancelFlag::default(), &|_| {})
                .await,
        );
        assert_eq!(error.code, AssistantCode::TurnLimitExceeded);
    }

    #[tokio::test]
    async fn cancel_closes_a_silent_stream() {
        // O servidor manda o começo e fica mudo, com a conexão aberta
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = [0u8; 4096];
            let _ = socket.read(&mut buffer).await;
            let _ = socket
                .write_all(b"HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\n\r\nevent: attempt\ndata: {\"attemptId\":\"a\",\"index\":0}\n\n")
                .await;
            tokio::time::sleep(Duration::from_secs(30)).await;
        });

        let cancel = CancelFlag::default();
        let trigger = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            trigger.cancel();
        });
        let started = std::time::Instant::now();
        let result = ApiClient::new(&base)
            .unwrap()
            .create_attempt("tok", TURN, &attempt_request(), &cancel, &|_| {})
            .await;
        assert!(matches!(result, Err(Error::Cancelled)), "{result:?}");
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn ids_in_paths_must_be_uuids() {
        assert!(path_segment(TURN).is_ok());
        assert!(path_segment("../admin").is_err());
        assert!(path_segment("6f1c2b8e-4d3a-4e5f-9a7b-1c2d3e4f5a6/").is_err());
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
