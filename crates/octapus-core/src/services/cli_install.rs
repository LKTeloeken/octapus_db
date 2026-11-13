//! Instalar o `octapus` de terminal a partir do app: baixa da release desta
//! mesma versão o binário do sistema, confere o SHA-256 publicado ao lado e
//! o grava na pasta de binários do usuário (sem pedir senha de administrador).
//!
//! O app e o terminal abrem o mesmo `app.db`, então andam na mesma versão: o
//! botão baixa sempre a release do próprio app, nunca "a mais nova".
//!
//! O `octapus update` usa o mesmo download para trocar o próprio binário pela
//! release mais nova ([`latest_version`] + [`install_to`]).

use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::error::{Error, Result};
use crate::models::CliStatus;

/// As releases do Octapus no GitHub; cada versão é a tag `app-v<versão>`.
const RELEASES: &str = "https://github.com/LKTeloeken/octapus_db/releases/download";
/// O manifesto da release mais nova publicada (o mesmo que o updater do app
/// consulta): rascunho e pré-release não entram nele.
const LATEST: &str = "https://github.com/LKTeloeken/octapus_db/releases/latest/download/latest.json";
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);

/// O nome do binário desta plataforma na release (`release.yml`), ou `None`
/// onde nenhum é publicado.
pub fn asset_name() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => Some("octapus-macos-arm64"),
        ("macos", "x86_64") => Some("octapus-macos-x64"),
        ("linux", "x86_64") => Some("octapus-linux-x86_64"),
        ("windows", "x86_64") => Some("octapus-windows-x64.exe"),
        _ => None,
    }
}

/// Onde o `octapus` é instalado: `~/.local/bin` no Unix (a pasta de binários
/// do usuário) e `%LOCALAPPDATA%\Programs\octapus` no Windows.
pub fn install_path() -> Result<PathBuf> {
    let missing = || Error::External("Não foi possível achar a pasta do usuário.".into());
    if cfg!(windows) {
        return Ok(dirs::data_local_dir().ok_or_else(missing)?.join("Programs/octapus/octapus.exe"));
    }
    Ok(dirs::home_dir().ok_or_else(missing)?.join(".local/bin/octapus"))
}

fn on_path(dir: &Path) -> bool {
    std::env::var_os("PATH").is_some_and(|path| std::env::split_paths(&path).any(|entry| entry == dir))
}

pub fn status() -> Result<CliStatus> {
    let path = install_path()?;
    Ok(CliStatus {
        supported: asset_name().is_some(),
        installed: path.is_file(),
        on_path: path.parent().is_some_and(on_path),
        path: path.display().to_string(),
    })
}

/// O SHA-256 do arquivo `.sha256` da release: o primeiro campo da linha.
fn parse_checksum(text: &str) -> Option<String> {
    let digest = text.split_whitespace().next()?.to_ascii_lowercase();
    (digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())).then_some(digest)
}

fn verify(bytes: &[u8], expected: &str) -> Result<()> {
    let actual = hex::encode(Sha256::digest(bytes));
    if actual != expected {
        return Err(Error::External(
            "O arquivo baixado não confere com o SHA-256 publicado na release; nada foi instalado.".into(),
        ));
    }
    Ok(())
}

/// Grava o binário em `path` (via temporário + rename, com permissão de
/// execução): quem estiver usando o `octapus` antigo segue com ele até fechar.
fn write_binary(path: &Path, bytes: &[u8]) -> Result<()> {
    let failed = |error: std::io::Error| Error::External(format!("Não foi possível gravar {}: {error}", path.display()));
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(failed)?;
    }
    let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
    std::fs::write(&tmp, bytes).map_err(failed)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755)).map_err(failed)?;
    }
    // O Windows não deixa gravar por cima de um executável em uso (o
    // `octapus update` trocando a si mesmo), mas deixa renomeá-lo
    #[cfg(windows)]
    if path.exists() {
        let old = path.with_extension("old");
        let _ = std::fs::remove_file(&old);
        let _ = std::fs::rename(path, &old);
    }
    std::fs::rename(&tmp, path).map_err(|error| {
        let _ = std::fs::remove_file(&tmp);
        failed(error)
    })
}

fn client(timeout: Duration) -> Result<reqwest::Client> {
    // O reqwest vem sem provedor de criptografia (como no cliente do assistente)
    if rustls::crypto::CryptoProvider::get_default().is_none() {
        let _ = rustls::crypto::ring::default_provider().install_default();
    }
    reqwest::Client::builder()
        .user_agent(concat!("octapus-db/", env!("CARGO_PKG_VERSION")))
        .timeout(timeout)
        .build()
        .map_err(|error| Error::External(format!("Não foi possível baixar o octapus: {error}")))
}

async fn download(client: &reqwest::Client, url: &str) -> Result<Vec<u8>> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| Error::External(format!("Não foi possível baixar o octapus: {error}")))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(Error::External(
            "Esta versão não tem o octapus de terminal publicado na release.".into(),
        ));
    }
    let response = response
        .error_for_status()
        .map_err(|error| Error::External(format!("Não foi possível baixar o octapus: {error}")))?;
    let bytes = response
        .bytes()
        .await
        .map_err(|error| Error::External(format!("O download do octapus foi interrompido: {error}")))?;
    Ok(bytes.to_vec())
}

/// Baixa e instala o `octapus` da release `version` (a do app).
pub async fn install(version: &str) -> Result<CliStatus> {
    install_to(version, &install_path()?).await?;
    status()
}

/// Baixa o `octapus` da release `version`, confere o SHA-256 e o grava em
/// `path` (o `octapus update` passa o caminho do próprio binário).
pub async fn install_to(version: &str, path: &Path) -> Result<()> {
    install_from(RELEASES, version, path).await
}

async fn install_from(releases: &str, version: &str, path: &Path) -> Result<()> {
    let asset = asset_name()
        .ok_or_else(|| Error::External("Não há octapus de terminal publicado para este sistema.".into()))?;
    let client = client(DOWNLOAD_TIMEOUT)?;

    let base = format!("{releases}/app-v{version}/{asset}");
    let checksum = download(&client, &format!("{base}.sha256")).await?;
    let expected = parse_checksum(&String::from_utf8_lossy(&checksum))
        .ok_or_else(|| Error::External("O SHA-256 publicado na release está ilegível; nada foi instalado.".into()))?;
    let bytes = download(&client, &base).await?;
    verify(&bytes, &expected)?;

    write_binary(path, &bytes)?;
    log::info!("octapus de terminal {version} instalado ({} bytes)", bytes.len());
    Ok(())
}

/// A versão da release mais nova publicada. `timeout` curto para quem só
/// quer avisar (a TUI); o `octapus update` espera mais.
pub async fn latest_version(timeout: Duration) -> Result<semver::Version> {
    latest_from(LATEST, timeout).await
}

async fn latest_from(url: &str, timeout: Duration) -> Result<semver::Version> {
    let unreadable = || Error::External("Não foi possível ler a versão mais nova do octapus.".into());
    let manifest = download(&client(timeout)?, url).await?;
    let manifest: serde_json::Value = serde_json::from_slice(&manifest).map_err(|_| unreadable())?;
    let version = manifest.get("version").and_then(|version| version.as_str()).ok_or_else(unreadable)?;
    semver::Version::parse(version.trim_start_matches('v')).map_err(|_| unreadable())
}

/// `latest` é mais nova do que a versão `current` (a deste binário)? Uma
/// versão que não dá para ler nunca pede atualização.
pub fn is_newer(latest: &semver::Version, current: &str) -> bool {
    semver::Version::parse(current).is_ok_and(|current| *latest > current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_file_is_read_strictly() {
        let digest = "a".repeat(64);
        assert_eq!(parse_checksum(&format!("{digest}\n")), Some(digest.clone()));
        assert_eq!(parse_checksum(&format!("{}  octapus-macos-arm64\n", digest.to_uppercase())), Some(digest));
        assert_eq!(parse_checksum("<html>Not Found</html>"), None);
        assert_eq!(parse_checksum(&"a".repeat(63)), None);
        assert_eq!(parse_checksum(""), None);
    }

    #[test]
    fn a_wrong_download_is_never_installed() {
        let expected = hex::encode(Sha256::digest(b"octapus"));
        assert!(verify(b"octapus", &expected).is_ok());
        assert_eq!(verify(b"outro", &expected).unwrap_err().code(), "EXTERNAL_ERROR");
    }

    #[test]
    fn binary_is_written_executable_and_replaced_whole() {
        let dir = std::env::temp_dir().join(format!("octapus-cli-install-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("bin/octapus");

        write_binary(&path, b"v1").unwrap();
        write_binary(&path, b"versao 2").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"versao 2");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o755);
        }
        // Nenhum temporário sobra ao lado
        assert_eq!(std::fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn only_a_higher_version_asks_for_an_update() {
        let latest = semver::Version::parse("1.2.0").unwrap();
        assert!(is_newer(&latest, "1.1.0"));
        assert!(is_newer(&latest, "1.2.0-beta.3"), "o beta sai para a versão final");
        assert!(!is_newer(&latest, "1.2.0"));
        assert!(!is_newer(&latest, "1.3.0"));
        assert!(!is_newer(&latest, "desconhecida"));
    }

    /// Um servidor HTTP de um arquivo por caminho, no lugar do GitHub.
    async fn serve(files: Vec<(String, Vec<u8>)>) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let mut request = vec![0; 4096];
                let read = socket.read(&mut request).await.unwrap_or(0);
                let request = String::from_utf8_lossy(&request[..read]).into_owned();
                let path = request.split_whitespace().nth(1).unwrap_or("/");
                let response = match files.iter().find(|(name, _)| name == path) {
                    Some((_, body)) => {
                        let mut head = format!("HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n", body.len()).into_bytes();
                        head.extend_from_slice(body);
                        head
                    }
                    None => b"HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\nconnection: close\r\n\r\n".to_vec(),
                };
                let _ = socket.write_all(&response).await;
            }
        });
        format!("http://{address}")
    }

    #[tokio::test]
    async fn update_installs_the_latest_release_and_refuses_a_tampered_one() {
        let Some(asset) = asset_name() else {
            return;
        };
        let binary = b"octapus novo".to_vec();
        let digest = hex::encode(Sha256::digest(&binary));
        let host = serve(vec![
            ("/latest.json".into(), br#"{ "version": "9.9.9", "platforms": {} }"#.to_vec()),
            (format!("/app-v9.9.9/{asset}"), binary.clone()),
            (format!("/app-v9.9.9/{asset}.sha256"), format!("{digest}\n").into_bytes()),
            (format!("/app-v6.6.6/{asset}"), b"adulterado".to_vec()),
            (format!("/app-v6.6.6/{asset}.sha256"), format!("{digest}\n").into_bytes()),
        ])
        .await;
        let dir = std::env::temp_dir().join(format!("octapus-cli-update-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("octapus");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(&path, b"octapus velho").unwrap();

        let latest = latest_from(&format!("{host}/latest.json"), Duration::from_secs(5)).await.unwrap();
        assert_eq!(latest.to_string(), "9.9.9");

        // O que não confere com o SHA-256 não substitui o binário
        let tampered = install_from(&host, "6.6.6", &path).await.unwrap_err();
        assert_eq!(tampered.code(), "EXTERNAL_ERROR");
        assert_eq!(std::fs::read(&path).unwrap(), b"octapus velho");
        // Versão sem binário publicado: erro, e nada muda
        assert!(install_from(&host, "1.0.0", &path).await.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"octapus velho");

        install_from(&host, &latest.to_string(), &path).await.unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), binary);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn this_platform_has_a_published_binary_and_a_user_level_path() {
        // As plataformas da release (release.yml); num sistema fora delas o
        // botão fica desligado em vez de baixar o binário errado
        if cfg!(any(
            all(target_os = "macos", any(target_arch = "aarch64", target_arch = "x86_64")),
            all(target_os = "linux", target_arch = "x86_64"),
            all(target_os = "windows", target_arch = "x86_64"),
        )) {
            assert!(asset_name().is_some());
        }
        let status = status().unwrap();
        assert!(status.path.contains("octapus"));
    }
}
