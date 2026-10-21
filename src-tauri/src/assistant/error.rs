//! Erros do assistente: a API fora do ar e os códigos que ela devolve no
//! corpo `{ "error": { "code", "message" } }` (contrato, `ErrorCode`). Cada um
//! vira um código `ASSISTANT_*` do `crate::Error`, e o front decide por ele.

use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Deserialize;

/// Os códigos da API, mais `Unavailable` também para a API inalcançável
/// (rede, DNS, tempo esgotado, proxy sem o corpo do contrato) — C-19.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssistantCode {
    InvalidRequest,
    Unauthorized,
    ConsentRequired,
    ModelSelectionForbidden,
    NotFound,
    Conflict,
    PromptNotRegistered,
    QuotaExceeded,
    TurnLimitExceeded,
    ModelUnavailable,
    InvalidOutput,
    ModelRefused,
    ProviderError,
    RateLimited,
    Unavailable,
    Internal,
}

impl AssistantCode {
    pub const ALL: [Self; 16] = [
        Self::InvalidRequest,
        Self::Unauthorized,
        Self::ConsentRequired,
        Self::ModelSelectionForbidden,
        Self::NotFound,
        Self::Conflict,
        Self::PromptNotRegistered,
        Self::QuotaExceeded,
        Self::TurnLimitExceeded,
        Self::ModelUnavailable,
        Self::InvalidOutput,
        Self::ModelRefused,
        Self::ProviderError,
        Self::RateLimited,
        Self::Unavailable,
        Self::Internal,
    ];

    /// O código como a API escreve (`unauthorized`); também o prefixo do
    /// `Display` (`Assistant unauthorized: …`).
    pub fn api_name(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::Unauthorized => "unauthorized",
            Self::ConsentRequired => "consent_required",
            Self::ModelSelectionForbidden => "model_selection_forbidden",
            Self::NotFound => "not_found",
            Self::Conflict => "conflict",
            Self::PromptNotRegistered => "prompt_not_registered",
            Self::QuotaExceeded => "quota_exceeded",
            Self::TurnLimitExceeded => "turn_limit_exceeded",
            Self::ModelUnavailable => "model_unavailable",
            Self::InvalidOutput => "invalid_output",
            Self::ModelRefused => "model_refused",
            Self::ProviderError => "provider_error",
            Self::RateLimited => "rate_limited",
            Self::Unavailable => "unavailable",
            Self::Internal => "internal",
        }
    }

    /// O código do `crate::Error` (`ASSISTANT_UNAUTHORIZED`), espelhado no
    /// `BackendErrorCode` do front.
    pub fn app_code(self) -> &'static str {
        match self {
            Self::InvalidRequest => "ASSISTANT_INVALID_REQUEST",
            Self::Unauthorized => "ASSISTANT_UNAUTHORIZED",
            Self::ConsentRequired => "ASSISTANT_CONSENT_REQUIRED",
            Self::ModelSelectionForbidden => "ASSISTANT_MODEL_SELECTION_FORBIDDEN",
            Self::NotFound => "ASSISTANT_NOT_FOUND",
            Self::Conflict => "ASSISTANT_CONFLICT",
            Self::PromptNotRegistered => "ASSISTANT_PROMPT_NOT_REGISTERED",
            Self::QuotaExceeded => "ASSISTANT_QUOTA_EXCEEDED",
            Self::TurnLimitExceeded => "ASSISTANT_TURN_LIMIT_EXCEEDED",
            Self::ModelUnavailable => "ASSISTANT_MODEL_UNAVAILABLE",
            Self::InvalidOutput => "ASSISTANT_INVALID_OUTPUT",
            Self::ModelRefused => "ASSISTANT_MODEL_REFUSED",
            Self::ProviderError => "ASSISTANT_PROVIDER_ERROR",
            Self::RateLimited => "ASSISTANT_RATE_LIMITED",
            Self::Unavailable => "ASSISTANT_UNAVAILABLE",
            Self::Internal => "ASSISTANT_INTERNAL",
        }
    }

    pub fn from_api_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|code| code.api_name() == name)
    }

    /// Resposta sem o corpo do contrato (proxy na frente da API, código novo
    /// que esta versão do app não conhece): decide pelo status HTTP.
    pub fn from_status(status: u16) -> Self {
        match status {
            400 | 422 => Self::InvalidRequest,
            401 => Self::Unauthorized,
            402 => Self::QuotaExceeded,
            404 => Self::NotFound,
            409 => Self::Conflict,
            429 => Self::RateLimited,
            502..=504 => Self::Unavailable,
            _ => Self::Internal,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AssistantError {
    pub code: AssistantCode,
    /// Texto da API (feito para log) ou da falha de rede; o front traduz pelo
    /// código. Não vai para o log do app.
    pub message: String,
    /// `Retry-After` de um 429: quanto esperar antes de tentar de novo.
    pub retry_after: Option<Duration>,
}

impl AssistantError {
    pub fn new(code: AssistantCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            retry_after: None,
        }
    }
}

#[derive(Deserialize)]
struct ErrorBody {
    error: ErrorDetail,
}

#[derive(Deserialize)]
struct ErrorDetail {
    code: String,
    message: String,
}

/// Monta o erro de uma resposta HTTP de falha: o `code` do corpo quando ele
/// segue o contrato, senão o status; mais o `Retry-After`, se veio.
pub fn from_response(
    status: u16,
    retry_after: Option<&str>,
    body: &str,
    now: DateTime<Utc>,
) -> AssistantError {
    let detail = serde_json::from_str::<ErrorBody>(body).ok().map(|body| body.error);
    let code = detail
        .as_ref()
        .and_then(|detail| AssistantCode::from_api_name(&detail.code))
        .unwrap_or_else(|| AssistantCode::from_status(status));
    let message = match detail {
        Some(detail) if !detail.message.is_empty() => detail.message,
        _ => format!("HTTP {status}"),
    };

    AssistantError {
        code,
        message,
        retry_after: retry_after.and_then(|value| parse_retry_after(value, now)),
    }
}

/// `Retry-After` em segundos (o que a API manda) ou como data HTTP (RFC 9110).
/// Data no passado vale zero.
pub fn parse_retry_after(value: &str, now: DateTime<Utc>) -> Option<Duration> {
    let value = value.trim();
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }
    let at = DateTime::parse_from_rfc2822(value).ok()?;
    Some((at.with_timezone(&Utc) - now).to_std().unwrap_or(Duration::ZERO))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-10-19T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn body(code: &str) -> String {
        serde_json::json!({ "error": { "code": code, "message": "texto da API" } }).to_string()
    }

    #[test]
    fn every_contract_code_maps_to_its_app_code() {
        // `ErrorCode` do contrato 1.5.0 (openapi.yaml) → código do app
        let expected = [
            ("invalid_request", 400, "ASSISTANT_INVALID_REQUEST"),
            ("unauthorized", 401, "ASSISTANT_UNAUTHORIZED"),
            ("consent_required", 403, "ASSISTANT_CONSENT_REQUIRED"),
            ("model_selection_forbidden", 403, "ASSISTANT_MODEL_SELECTION_FORBIDDEN"),
            ("not_found", 404, "ASSISTANT_NOT_FOUND"),
            ("conflict", 409, "ASSISTANT_CONFLICT"),
            ("prompt_not_registered", 422, "ASSISTANT_PROMPT_NOT_REGISTERED"),
            ("quota_exceeded", 402, "ASSISTANT_QUOTA_EXCEEDED"),
            ("turn_limit_exceeded", 409, "ASSISTANT_TURN_LIMIT_EXCEEDED"),
            ("model_unavailable", 503, "ASSISTANT_MODEL_UNAVAILABLE"),
            ("invalid_output", 200, "ASSISTANT_INVALID_OUTPUT"),
            ("model_refused", 200, "ASSISTANT_MODEL_REFUSED"),
            ("provider_error", 502, "ASSISTANT_PROVIDER_ERROR"),
            ("rate_limited", 429, "ASSISTANT_RATE_LIMITED"),
            ("unavailable", 503, "ASSISTANT_UNAVAILABLE"),
            ("internal", 500, "ASSISTANT_INTERNAL"),
        ];
        assert_eq!(expected.len(), AssistantCode::ALL.len());

        for (api_code, status, app_code) in expected {
            let error = from_response(status, None, &body(api_code), now());
            assert_eq!(error.code.app_code(), app_code, "{api_code}");
            assert_eq!(error.code.api_name(), api_code);
            assert_eq!(error.message, "texto da API");

            let app_error = crate::error::Error::Assistant(error);
            assert_eq!(app_error.code(), app_code);
            assert_eq!(
                app_error.to_string(),
                format!("Assistant {api_code}: texto da API")
            );
        }
    }

    #[test]
    fn body_outside_the_contract_falls_back_to_status() {
        let html = "<html>502 Bad Gateway</html>";
        let error = from_response(502, None, html, now());
        assert_eq!(error.code, AssistantCode::Unavailable);
        assert_eq!(error.message, "HTTP 502");

        assert_eq!(from_response(401, None, "", now()).code, AssistantCode::Unauthorized);
        assert_eq!(from_response(429, None, "", now()).code, AssistantCode::RateLimited);
        assert_eq!(from_response(500, None, "{}", now()).code, AssistantCode::Internal);
        assert_eq!(from_response(418, None, "", now()).code, AssistantCode::Internal);
    }

    #[test]
    fn unknown_api_code_falls_back_to_status() {
        let error = from_response(503, None, &body("brand_new_code"), now());
        assert_eq!(error.code, AssistantCode::Unavailable);
        assert_eq!(error.message, "texto da API");
    }

    #[test]
    fn reads_retry_after_in_seconds_or_as_date() {
        let error = from_response(429, Some("30"), &body("rate_limited"), now());
        assert_eq!(error.retry_after, Some(Duration::from_secs(30)));

        assert_eq!(
            parse_retry_after("Mon, 19 Oct 2026 12:01:30 GMT", now()),
            Some(Duration::from_secs(90))
        );
        // Data que já passou: tentar já
        assert_eq!(
            parse_retry_after("Mon, 19 Oct 2026 11:00:00 GMT", now()),
            Some(Duration::ZERO)
        );
        assert_eq!(parse_retry_after("soon", now()), None);
        assert_eq!(from_response(429, None, "", now()).retry_after, None);
    }

    #[test]
    fn not_configured_has_its_own_code() {
        let error = crate::error::Error::AssistantNotConfigured;
        assert_eq!(error.code(), "ASSISTANT_NOT_CONFIGURED");
        assert_eq!(error.to_string(), "Assistant not configured");
    }
}
