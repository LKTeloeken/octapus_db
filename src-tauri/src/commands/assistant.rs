use std::future::Future;

use tauri::State;

use crate::assistant::{ApiClient, AssistantCode, AssistantError};
use crate::error::{Error, Result};
use crate::models::{
    AssistantConsentTerm, AssistantMe, AssistantModel, AssistantParticipant, AssistantStatus,
};
use crate::state::AppState;
use crate::storage::repositories::assistant as assistant_store;

/// Configurado? A API está no ar? Há login? Nunca rejeita por causa da API:
/// sem URL ou com a API fora, o status diz isso e o resto do app segue (C-19).
#[tauri::command]
pub async fn assistant_status(state: State<'_, AppState>) -> Result<AssistantStatus> {
    let Ok((api_url, client)) = state.assistant.endpoint() else {
        return Ok(AssistantStatus::not_configured());
    };
    let logged_in = assistant_store::load_token(&state.storage, api_url)?.is_some();

    let health = match client.health().await {
        Ok(health) => Some(health),
        Err(error) => {
            log::info!("assistente: API fora do ar ({})", error.code());
            None
        }
    };
    let online = health.as_ref().is_some_and(|health| health.status == "ok");
    let min_app_version = health.and_then(|health| health.min_app_version);

    Ok(AssistantStatus {
        configured: true,
        online,
        logged_in,
        update_required: needs_update(min_app_version.as_deref(), env!("CARGO_PKG_VERSION")),
        min_app_version,
    })
}

/// Troca o código de convite por um token de participante e o guarda cifrado.
#[tauri::command]
pub async fn assistant_redeem(
    state: State<'_, AppState>,
    invite_code: String,
    device_label: Option<String>,
) -> Result<AssistantParticipant> {
    let (api_url, client) = state.assistant.endpoint()?;
    let redeemed = client
        .redeem(invite_code.trim(), device_label.as_deref())
        .await
        .inspect_err(|error| log::warn!("assistente: convite recusado ({})", error.code()))?;

    assistant_store::save_token(&state.storage, api_url, &redeemed.token)?;
    log::info!("assistente: login do participante {}", redeemed.participant.id);
    Ok(redeemed.participant)
}

/// Esquece o token nesta máquina (a API não tem logout).
#[tauri::command]
pub fn assistant_logout(state: State<'_, AppState>) -> Result<()> {
    assistant_store::clear(&state.storage)?;
    log::info!("assistente: logout");
    Ok(())
}

/// Participante, consentimento, cota e modo de seleção de modelo.
#[tauri::command]
pub async fn assistant_me(state: State<'_, AppState>) -> Result<AssistantMe> {
    authorized(&state, "me", |client, token| async move { client.me(&token).await }).await
}

/// Texto do termo vigente, para exibir antes do aceite (RN-11).
#[tauri::command]
pub async fn assistant_consent_current(state: State<'_, AppState>) -> Result<AssistantConsentTerm> {
    authorized(&state, "termo", |client, token| async move {
        client.consent_current(&token).await
    })
    .await
}

/// Aceita o termo `version` (precisa ser o vigente: outro → `ASSISTANT_CONFLICT`).
#[tauri::command]
pub async fn assistant_consent_accept(state: State<'_, AppState>, version: String) -> Result<()> {
    authorized(&state, "aceite do termo", |client, token| async move {
        client.consent_accept(&token, &version).await
    })
    .await?;
    log::info!("assistente: termo aceito");
    Ok(())
}

/// Modelos que o participante pode escolher — só no modo `free`; nos outros a
/// API responde `ASSISTANT_MODEL_SELECTION_FORBIDDEN`.
#[tauri::command]
pub async fn assistant_models(state: State<'_, AppState>) -> Result<Vec<AssistantModel>> {
    authorized(&state, "modelos", |client, token| async move {
        client.models(&token).await
    })
    .await
}

/// Chamada autenticada com o token salvo para a API atual. Sem token →
/// `ASSISTANT_UNAUTHORIZED`, o mesmo de um token recusado: para a tela, os dois
/// pedem o convite. Um 401 da API (token inválido, convite revogado) apaga o
/// token local, e o `assistant_status` volta a dizer "sem login".
async fn authorized<'a, T, F, Fut>(state: &'a AppState, operation: &str, call: F) -> Result<T>
where
    F: FnOnce(&'a ApiClient, String) -> Fut,
    Fut: Future<Output = Result<T>>,
{
    let (api_url, client) = state.assistant.endpoint()?;
    let token = assistant_store::load_token(&state.storage, api_url)?.ok_or_else(|| {
        Error::from(AssistantError::new(AssistantCode::Unauthorized, "not signed in"))
    })?;

    let result = call(client, token).await;
    if let Err(error) = &result {
        log::warn!("assistente: {operation} falhou ({})", error.code());
        if matches!(error, Error::Assistant(e) if e.code == AssistantCode::Unauthorized) {
            assistant_store::clear(&state.storage)?;
            log::info!("assistente: token recusado pela API, sessão local apagada");
        }
    }
    result
}

/// Este app está abaixo da versão mínima que a API pede. Versão ilegível de
/// qualquer lado não bloqueia.
fn needs_update(min_app_version: Option<&str>, current: &str) -> bool {
    let (Some(min), Ok(current)) = (min_app_version, semver::Version::parse(current)) else {
        return false;
    };
    semver::Version::parse(min.trim_start_matches('v')).is_ok_and(|min| current < min)
}

#[cfg(test)]
mod tests {
    use super::needs_update;

    #[test]
    fn update_required_only_below_the_minimum() {
        assert!(needs_update(Some("1.2.0"), "1.1.0-beta.10"));
        assert!(needs_update(Some("1.1.0"), "1.1.0-beta.10"));
        assert!(!needs_update(Some("1.1.0-beta.10"), "1.1.0-beta.10"));
        assert!(!needs_update(Some("1.0.0"), "1.1.0-beta.10"));
        assert!(!needs_update(None, "1.1.0"));
        assert!(!needs_update(Some("garbage"), "1.1.0"));
    }
}
