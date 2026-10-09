use std::time::Duration;

use reqwest::header::ACCEPT;
use semver::Version;
use serde::{Deserialize, Serialize};
use tauri::{Manager, Url, Webview};
use tauri_plugin_updater::UpdaterExt;

/// Releases do repositório, as mais novas primeiro (inclui pre-releases; os
/// rascunhos não aparecem sem token).
const RELEASES_API: &str = "https://api.github.com/repos/LKTeloeken/octapus_db/releases?per_page=30";
/// Só manifestos das releases do próprio repositório: o updater baixaria e
/// instalaria o que o manifesto apontasse.
const RELEASE_ASSETS_PREFIX: &str = "https://github.com/LKTeloeken/octapus_db/releases/download/";
const MANIFEST_NAME: &str = "latest.json";
const TAG_PREFIX: &str = "app-v";
const API_TIMEOUT: Duration = Duration::from_secs(15);

/// Mesmo formato do `check` do plugin: o front monta um `Update` do
/// `@tauri-apps/plugin-updater` com isto e segue com o `downloadAndInstall` dele.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMetadata {
    rid: tauri::ResourceId,
    current_version: String,
    version: String,
    body: Option<String>,
    raw_json: serde_json::Value,
}

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    draft: bool,
    assets: Vec<GithubAsset>,
}

#[derive(Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

/// Checa atualização pelo canal beta: a versão mais alta entre releases e
/// pre-releases publicadas. O endpoint fixo do `tauri.conf.json`
/// (`releases/latest`) nunca enxerga uma pre-release — o GitHub só marca como
/// "latest" uma release normal —, então aqui o manifesto vem da API.
/// A assinatura continua verificada com a chave pública da config.
#[tauri::command]
pub async fn check_beta_update(webview: Webview) -> Result<Option<UpdateMetadata>, String> {
    let releases = fetch_releases().await?;

    let mut builder = webview.updater_builder();
    // Nenhuma release com manifesto: fica o endpoint estável da config
    if let Some((_, manifest)) = newest_manifest(&releases) {
        builder = builder
            .endpoints(vec![release_manifest_url(manifest)?])
            .map_err(|e| e.to_string())?;
    }

    let update = builder
        .build()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?;

    Ok(update.map(|update| UpdateMetadata {
        current_version: update.current_version.clone(),
        version: update.version.clone(),
        body: update.body.clone(),
        raw_json: update.raw_json.clone(),
        rid: webview.resources_table().add(update),
    }))
}

async fn fetch_releases() -> Result<Vec<GithubRelease>, String> {
    // O reqwest vem sem provedor de criptografia (como no plugin do updater,
    // que instala o mesmo `ring` na primeira checagem)
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }

    let client = reqwest::Client::builder()
        // A API do GitHub recusa pedidos sem User-Agent
        .user_agent(concat!("octapus-db/", env!("CARGO_PKG_VERSION")))
        .timeout(API_TIMEOUT)
        .build()
        .map_err(|e| e.to_string())?;

    client
        .get(RELEASES_API)
        .header(ACCEPT, "application/vnd.github+json")
        .send()
        .await
        .and_then(|response| response.error_for_status())
        .map_err(|e| format!("Falha ao listar as releases: {e}"))?
        .json()
        .await
        .map_err(|e| format!("Resposta inesperada da API de releases: {e}"))
}

/// A release publicada de versão mais alta que tem manifesto do updater.
fn newest_manifest(releases: &[GithubRelease]) -> Option<(Version, &str)> {
    releases
        .iter()
        .filter(|release| !release.draft)
        .filter_map(|release| {
            let version = release.tag_name.strip_prefix(TAG_PREFIX)?;
            let version = Version::parse(version).ok()?;
            let manifest = release.assets.iter().find(|asset| asset.name == MANIFEST_NAME)?;
            Some((version, manifest.browser_download_url.as_str()))
        })
        .max_by(|a, b| a.0.cmp(&b.0))
}

fn release_manifest_url(raw: &str) -> Result<Url, String> {
    let url = Url::parse(raw).map_err(|e| e.to_string())?;
    let path_ok = url
        .as_str()
        .strip_prefix(RELEASE_ASSETS_PREFIX)
        .and_then(|rest| rest.strip_suffix(MANIFEST_NAME))
        // Exatamente `<tag>/latest.json`: nada de `..` ou subpastas
        .and_then(|tag| tag.strip_suffix('/'))
        .is_some_and(|tag| !tag.is_empty() && !tag.contains('/') && tag != "..");
    if !path_ok || url.query().is_some() || url.fragment().is_some() {
        return Err(format!("Endpoint de atualização não permitido: {raw}"));
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str, draft: bool, with_manifest: bool) -> GithubRelease {
        let assets = with_manifest
            .then(|| GithubAsset {
                name: MANIFEST_NAME.into(),
                browser_download_url: format!("{RELEASE_ASSETS_PREFIX}{tag}/{MANIFEST_NAME}"),
            })
            .into_iter()
            .collect();
        GithubRelease {
            tag_name: tag.into(),
            draft,
            assets,
        }
    }

    #[test]
    fn picks_the_highest_published_version_with_a_manifest() {
        let releases = [
            release("app-v1.0.2", false, true),
            release("app-v1.2.0-beta.1", true, true),   // rascunho
            release("app-v1.1.5", false, false),        // sem latest.json
            release("app-v1.1.0-beta.2", false, true),
            release("app-v1.1.0-beta.10", false, true), // semver, não ordem de texto
            release("nix-1.0", false, true),            // tag fora do padrão
        ];

        let (version, url) = newest_manifest(&releases).unwrap();
        assert_eq!(version.to_string(), "1.1.0-beta.10");
        assert!(url.ends_with("app-v1.1.0-beta.10/latest.json"));
    }

    #[test]
    fn a_stable_release_beats_its_own_betas() {
        let releases = [
            release("app-v1.1.0-beta.3", false, true),
            release("app-v1.1.0", false, true),
        ];
        assert_eq!(newest_manifest(&releases).unwrap().0.to_string(), "1.1.0");
        assert!(newest_manifest(&[]).is_none());
    }

    #[test]
    fn accepts_release_manifests_only() {
        assert!(release_manifest_url(
            "https://github.com/LKTeloeken/octapus_db/releases/download/app-v1.1.0-beta.1/latest.json"
        )
        .is_ok());

        for bad in [
            "https://example.com/latest.json",
            "http://github.com/LKTeloeken/octapus_db/releases/download/app-v1/latest.json",
            "https://github.com/other/repo/releases/download/app-v1/latest.json",
            "https://github.com/LKTeloeken/octapus_db/releases/download/latest.json",
            "https://github.com/LKTeloeken/octapus_db/releases/download/a/b/latest.json",
            "https://github.com/LKTeloeken/octapus_db/releases/download/app-v1/latest.json?x=1",
            "https://github.com/LKTeloeken/octapus_db/releases/download/app-v1/other.json",
        ] {
            assert!(release_manifest_url(bad).is_err(), "{bad}");
        }
    }
}
