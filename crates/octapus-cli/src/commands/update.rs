//! `octapus update`: troca este binário pela release mais nova, com o mesmo
//! download conferido por SHA-256 do botão do app (`services/cli_install`).
//!
//! A TUI avisa quando há versão nova ([`newer_version`]); a consulta vai à
//! rede no máximo uma vez por dia e nunca acontece nos subcomandos nem no
//! MCP — a saída deles é dado.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use octapus_core::paths;
use octapus_core::services::cli_install;
use serde_json::{json, Value};

use crate::error::CliResult;
use crate::output::Output;

const CURRENT: &str = env!("CARGO_PKG_VERSION");
/// Quem pediu o update espera; quem só abre a TUI, não.
const UPDATE_TIMEOUT: Duration = Duration::from_secs(20);
const NOTICE_TIMEOUT: Duration = Duration::from_secs(5);
/// O aviso da TUI reaproveita a última resposta por um dia.
const CHECK_EVERY: Duration = Duration::from_secs(24 * 60 * 60);
const CHECK_FILE: &str = "cli-update-check.json";

pub async fn run(out: &Output, check: bool) -> CliResult<()> {
    let latest = cli_install::latest_version(UPDATE_TIMEOUT).await?;
    let newer = cli_install::is_newer(&latest, CURRENT);
    let latest = latest.to_string();
    if let Ok(dir) = paths::data_dir() {
        remember(&dir, &latest);
    }

    if !newer {
        return out.done(
            &format!("octapus {CURRENT} já é a versão mais nova"),
            json!({ "current": CURRENT, "latest": latest, "updated": false }),
        );
    }
    if check {
        return out.done(
            &format!("octapus {latest} disponível (instalado: {CURRENT}): rode octapus update"),
            json!({ "current": CURRENT, "latest": latest, "updated": false }),
        );
    }

    // O binário em uso, seguindo um link simbólico até o arquivo de verdade
    let path = std::env::current_exe().and_then(|path| path.canonicalize())?;
    cli_install::install_to(&latest, &path).await?;
    log::info!("octapus atualizado de {CURRENT} para {latest}");
    out.done(
        &format!("octapus atualizado de {CURRENT} para {latest} ({})", path.display()),
        json!({ "current": CURRENT, "latest": latest, "updated": true, "path": path }),
    )
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_secs())
}

fn check_file(dir: &Path) -> PathBuf {
    dir.join(CHECK_FILE)
}

/// A versão mais nova vista na última consulta, se ela ainda vale.
fn remembered(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(check_file(dir)).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    let checked_at = value.get("checkedAt")?.as_u64()?;
    (now().saturating_sub(checked_at) < CHECK_EVERY.as_secs())
        .then(|| value.get("latest")?.as_str().map(str::to_string))
        .flatten()
}

/// Guarda a resposta (temporário com o pid + rename: outro `octapus` pode
/// estar gravando o mesmo arquivo).
fn remember(dir: &Path, latest: &str) {
    let path = check_file(dir);
    let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
    let body = json!({ "checkedAt": now(), "latest": latest }).to_string();
    if std::fs::write(&tmp, body).is_ok() && std::fs::rename(&tmp, &path).is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
}

fn newer_than_current(latest: &str) -> Option<String> {
    let version = latest.parse().ok()?;
    cli_install::is_newer(&version, CURRENT).then(|| latest.to_string())
}

/// A versão mais nova, se for maior que a deste binário. Sem rede, ou com
/// qualquer erro, não há aviso (e nada aparece na tela).
pub async fn newer_version() -> Option<String> {
    let dir = paths::data_dir().ok()?;
    if let Some(latest) = remembered(&dir) {
        return newer_than_current(&latest);
    }
    let latest = cli_install::latest_version(NOTICE_TIMEOUT).await.ok()?.to_string();
    remember(&dir, &latest);
    newer_than_current(&latest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_answer_is_reused_for_a_day() {
        let dir = std::env::temp_dir().join(format!("octapus-update-check-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(remembered(&dir), None);

        remember(&dir, "99.0.0");
        assert_eq!(remembered(&dir).as_deref(), Some("99.0.0"));
        assert_eq!(newer_than_current("99.0.0").as_deref(), Some("99.0.0"));
        assert_eq!(newer_than_current(CURRENT), None);
        assert_eq!(newer_than_current("lixo"), None);

        // Velha demais, ou ilegível: consulta de novo
        let old = json!({ "checkedAt": now() - CHECK_EVERY.as_secs() - 1, "latest": "99.0.0" });
        std::fs::write(check_file(&dir), old.to_string()).unwrap();
        assert_eq!(remembered(&dir), None);
        std::fs::write(check_file(&dir), "{").unwrap();
        assert_eq!(remembered(&dir), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
