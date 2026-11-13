//! Onde estão o `pg_dump`/`pg_restore`/`psql` e se servem para um servidor.
//!
//! Ordem de busca: a pasta das Preferências (se houver, é a única que vale),
//! o `PATH` e os locais de instalação conhecidos. Os locais conhecidos importam
//! porque um app aberto pelo Finder recebe só `/usr/bin:/bin:/usr/sbin:/sbin`
//! no `PATH` — o Homebrew e o Postgres.app ficam de fora.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::models::{
    PgToolsDetection, PgToolsInstall, PgToolsProblem, PgToolsSource, PgToolsStatus,
};

use super::tool_command;

/// Um `--version` que não responde nisso é tratado como binário quebrado.
const VERSION_TIMEOUT: Duration = Duration::from_secs(5);

const PG_DUMP: &str = "pg_dump";
const PG_RESTORE: &str = "pg_restore";
const PSQL: &str = "psql";

/// Procura as ferramentas. `configured` é a pasta das Preferências: quando
/// existe, é a escolhida — se não servir, o erro volta em `configured_error`
/// em vez de cair em outra instalação sem o usuário saber.
pub async fn detect(configured: Option<&str>) -> PgToolsDetection {
    let configured = configured.map(str::trim).filter(|dir| !dir.is_empty());

    let mut seen = HashSet::new();
    let mut candidates = Vec::new();
    for (dir, source) in search_dirs() {
        if let Ok(install) = inspect_dir(&dir, source).await {
            // O mesmo binário aparece por vários caminhos (no Homebrew,
            // `bin/` e `opt/postgresql@16/bin` apontam para o Cellar)
            let real = std::fs::canonicalize(&install.pg_dump)
                .unwrap_or_else(|_| PathBuf::from(&install.pg_dump));
            if seen.insert(real) {
                candidates.push(install);
            }
        }
    }
    // Estável: no empate de versão, o que veio antes (PATH) fica na frente
    candidates.sort_by_key(|install| std::cmp::Reverse(install.version_num));

    match configured {
        Some(dir) => match inspect_dir(Path::new(dir), PgToolsSource::Configured).await {
            Ok(install) => PgToolsDetection {
                selected: Some(install),
                candidates,
                configured_error: None,
            },
            Err(message) => PgToolsDetection {
                selected: None,
                candidates,
                configured_error: Some(message),
            },
        },
        None => PgToolsDetection {
            selected: candidates.first().cloned(),
            candidates,
            configured_error: None,
        },
    }
}

/// As ferramentas frente a um servidor (versão no formato do
/// `server_version_num`). O `pg_dump` se recusa a ler um servidor de major
/// mais nova; o contrário funciona.
pub fn status(
    detection: PgToolsDetection,
    server_version: String,
    server_version_num: u32,
) -> PgToolsStatus {
    let problem = if let Some(message) = &detection.configured_error {
        Some(PgToolsProblem::InvalidConfigured {
            message: message.clone(),
        })
    } else {
        match &detection.selected {
            None => Some(PgToolsProblem::NotFound),
            Some(install) if major(install.version_num) < major(server_version_num) => {
                Some(PgToolsProblem::ClientTooOld {
                    client: install.version.clone(),
                    server: server_version.clone(),
                })
            }
            Some(_) => None,
        }
    };

    PgToolsStatus {
        detection,
        server_version,
        server_version_num,
        problem,
    }
}

/// Lê o par (`server_version_num`, `server_version`) do servidor. O texto
/// perde o sufixo da distribuição (`16.4 (Debian 16.4-1.pgdg120+1)` → `16.4`);
/// sem o número (servidor estranho), ele sai do texto.
pub fn version_from_server_setting(num: Option<&str>, text: Option<&str>) -> (String, u32) {
    let version = text
        .and_then(|text| text.split_whitespace().next())
        .unwrap_or_default()
        .to_string();
    let num = num
        .and_then(|num| num.trim().parse().ok())
        .or_else(|| parse_version_token(&version))
        .unwrap_or(0);
    (version, num)
}

/// Major de um número no formato do `server_version_num`: 160004 → 16,
/// 90624 → 906 (antes da 10, a major tinha dois números).
fn major(num: u32) -> u32 {
    if num >= 100_000 {
        num / 10_000
    } else {
        num / 100
    }
}

/// Extrai a versão da saída do `--version` (`pg_dump (PostgreSQL) 16.4
/// (Homebrew)`): o primeiro pedaço que começa com dígito.
fn parse_version_output(output: &str) -> Option<(String, u32)> {
    let token = output
        .split_whitespace()
        .find(|token| token.starts_with(|c: char| c.is_ascii_digit()))?;
    let num = parse_version_token(token)?;
    Some((token.to_string(), num))
}

/// `16.4` → 160004, `17beta1` → 170000, `9.6.24` → 90624.
fn parse_version_token(token: &str) -> Option<u32> {
    let mut parts = token.split('.').map(|part| {
        let digits: String = part.chars().take_while(char::is_ascii_digit).collect();
        digits.parse::<u32>().ok()
    });
    let first = parts.next()??;
    let second = parts.next().flatten().unwrap_or(0);
    if first >= 10 {
        Some(first * 10_000 + second)
    } else {
        let third = parts.next().flatten().unwrap_or(0);
        Some(first * 10_000 + second * 100 + third)
    }
}

/// Confere uma pasta: precisa do `pg_dump` executável e de um `--version`
/// legível; `pg_restore` e `psql` são anotados se existirem.
async fn inspect_dir(dir: &Path, source: PgToolsSource) -> Result<PgToolsInstall, String> {
    if !dir.is_dir() {
        return Err(format!("A pasta {} não existe.", dir.display()));
    }
    let pg_dump = executable_in(dir, PG_DUMP)
        .ok_or_else(|| format!("Não há pg_dump em {}.", dir.display()))?;

    let output = tool_command(&pg_dump).arg("--version").output();
    let output = tokio::time::timeout(VERSION_TIMEOUT, output)
        .await
        .map_err(|_| format!("{} não respondeu ao --version.", pg_dump.display()))?
        .map_err(|e| format!("Não foi possível executar {}: {e}", pg_dump.display()))?;
    let (version, version_num) = parse_version_output(&String::from_utf8_lossy(&output.stdout))
        .ok_or_else(|| format!("Versão ilegível em {}.", pg_dump.display()))?;

    Ok(PgToolsInstall {
        dir: dir.display().to_string(),
        source,
        version,
        version_num,
        pg_dump: pg_dump.display().to_string(),
        pg_restore: executable_in(dir, PG_RESTORE).map(|p| p.display().to_string()),
        psql: executable_in(dir, PSQL).map(|p| p.display().to_string()),
    })
}

fn executable_in(dir: &Path, name: &str) -> Option<PathBuf> {
    let path = dir.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    is_executable(&path).then_some(path)
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.metadata()
        .map(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

/// Pastas onde procurar, na ordem: `PATH`, depois os locais conhecidos.
fn search_dirs() -> Vec<(PathBuf, PgToolsSource)> {
    let mut dirs: Vec<(PathBuf, PgToolsSource)> = std::env::var_os("PATH")
        .map(|path| {
            std::env::split_paths(&path)
                .map(|dir| (dir, PgToolsSource::Path))
                .collect()
        })
        .unwrap_or_default();
    for pattern in known_locations() {
        dirs.extend(
            expand_star(&pattern)
                .into_iter()
                .map(|dir| (dir, PgToolsSource::KnownLocation)),
        );
    }
    dirs
}

/// Locais em que os instaladores mais comuns deixam as ferramentas. `*` vale
/// por um nome de pasta (a versão, quase sempre).
fn known_locations() -> Vec<PathBuf> {
    let mut patterns: Vec<PathBuf> = Vec::new();

    #[cfg(target_os = "macos")]
    patterns.extend(
        [
            // Homebrew (Apple Silicon e Intel), com e sem versão no nome
            "/opt/homebrew/bin",
            "/opt/homebrew/opt/postgresql@*/bin",
            "/opt/homebrew/opt/libpq/bin",
            "/usr/local/bin",
            "/usr/local/opt/postgresql@*/bin",
            "/usr/local/opt/libpq/bin",
            // Postgres.app, instalador da EDB e MacPorts
            "/Applications/Postgres.app/Contents/Versions/*/bin",
            "/Library/PostgreSQL/*/bin",
            "/opt/local/lib/postgresql*/bin",
        ]
        .map(PathBuf::from),
    );

    #[cfg(target_os = "linux")]
    {
        patterns.extend(
            [
                "/usr/bin",
                "/usr/local/bin",
                // Debian/Ubuntu (pacotes do PGDG lado a lado) e RHEL/Fedora
                "/usr/lib/postgresql/*/bin",
                "/usr/pgsql-*/bin",
                // NixOS
                "/run/current-system/sw/bin",
            ]
            .map(PathBuf::from),
        );
        if let Some(home) = std::env::var_os("HOME") {
            patterns.push(PathBuf::from(home).join(".nix-profile/bin"));
        }
    }

    #[cfg(windows)]
    for var in ["ProgramFiles", "ProgramFiles(x86)"] {
        if let Some(base) = std::env::var_os(var) {
            patterns.push(PathBuf::from(base).join("PostgreSQL").join("*").join("bin"));
        }
    }

    patterns
}

/// Expande os componentes com `*` de um caminho (sem `**` nem `?`), só com as
/// pastas que existem.
fn expand_star(pattern: &Path) -> Vec<PathBuf> {
    let mut current = vec![PathBuf::new()];
    for component in pattern.components() {
        let part = component.as_os_str().to_string_lossy();
        let Some((prefix, suffix)) = part.split_once('*') else {
            for path in &mut current {
                path.push(component);
            }
            continue;
        };
        let mut next = Vec::new();
        for parent in &current {
            let Ok(entries) = std::fs::read_dir(parent) else {
                continue;
            };
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.len() >= prefix.len() + suffix.len()
                    && name.starts_with(prefix)
                    && name.ends_with(suffix)
                {
                    next.push(entry.path());
                }
            }
        }
        current = next;
    }
    current.retain(|path| path.is_dir());
    current
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_version_outputs() {
        let cases = [
            ("pg_dump (PostgreSQL) 16.4\n", "16.4", 160_004),
            ("pg_dump (PostgreSQL) 16.4 (Homebrew)\n", "16.4", 160_004),
            (
                "pg_dump (PostgreSQL) 15.8 (Ubuntu 15.8-1.pgdg22.04+1)",
                "15.8",
                150_008,
            ),
            ("pg_dump (PostgreSQL) 17beta1", "17beta1", 170_000),
            ("pg_dump (PostgreSQL) 18rc1", "18rc1", 180_000),
            ("pg_dump (PostgreSQL) 9.6.24", "9.6.24", 90_624),
        ];
        for (output, version, num) in cases {
            assert_eq!(
                parse_version_output(output),
                Some((version.to_string(), num)),
                "{output}"
            );
        }
        assert_eq!(parse_version_output("pg_dump: command not found"), None);
        assert_eq!(parse_version_output(""), None);
    }

    #[test]
    fn majors_follow_the_pre_and_post_10_schemes() {
        assert_eq!(major(160_004), 16);
        assert_eq!(major(100_000), 10);
        assert_eq!(major(90_624), 906);
        assert_eq!(major(90_200), 902);
        assert!(major(90_624) > major(90_224));
    }

    #[test]
    fn server_setting_drops_distribution_suffix() {
        assert_eq!(
            version_from_server_setting(Some("160004"), Some("16.4 (Debian 16.4-1.pgdg120+1)")),
            ("16.4".to_string(), 160_004)
        );
        // Sem o número (proxy/fork estranho), sai do texto
        assert_eq!(
            version_from_server_setting(None, Some("15.2")),
            ("15.2".to_string(), 150_002)
        );
        assert_eq!(version_from_server_setting(None, None), (String::new(), 0));
    }

    fn install(version: &str, num: u32) -> PgToolsInstall {
        PgToolsInstall {
            dir: "/x".into(),
            source: PgToolsSource::Path,
            version: version.into(),
            version_num: num,
            pg_dump: "/x/pg_dump".into(),
            pg_restore: Some("/x/pg_restore".into()),
            psql: Some("/x/psql".into()),
        }
    }

    fn detection(
        selected: Option<PgToolsInstall>,
        configured_error: Option<&str>,
    ) -> PgToolsDetection {
        PgToolsDetection {
            candidates: selected.iter().cloned().collect(),
            selected,
            configured_error: configured_error.map(String::from),
        }
    }

    #[test]
    fn status_flags_each_problem() {
        let ok = status(
            detection(Some(install("16.4", 160_004)), None),
            "16.1".into(),
            160_001,
        );
        assert_eq!(ok.problem, None);

        // Cliente mais novo que o servidor funciona
        let newer = status(
            detection(Some(install("17.0", 170_000)), None),
            "13.2".into(),
            130_002,
        );
        assert_eq!(newer.problem, None);

        let old = status(
            detection(Some(install("15.8", 150_008)), None),
            "16.4".into(),
            160_004,
        );
        assert_eq!(
            old.problem,
            Some(PgToolsProblem::ClientTooOld {
                client: "15.8".into(),
                server: "16.4".into()
            })
        );

        let missing = status(detection(None, None), "16.4".into(), 160_004);
        assert_eq!(missing.problem, Some(PgToolsProblem::NotFound));

        let invalid = status(
            detection(None, Some("Não há pg_dump em /y.")),
            "16.4".into(),
            160_004,
        );
        assert_eq!(
            invalid.problem,
            Some(PgToolsProblem::InvalidConfigured {
                message: "Não há pg_dump em /y.".into()
            })
        );
    }

    /// Pasta temporária própria do teste, apagada no fim.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("octapus-pgtools-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn expands_star_components() {
        let root = TempDir::new("expand");
        for dir in [
            "postgresql@15/bin",
            "postgresql@16/bin",
            "libpq/bin",
            "postgresql@17",
        ] {
            std::fs::create_dir_all(root.0.join(dir)).unwrap();
        }

        let mut found = expand_star(&root.0.join("postgresql@*").join("bin"));
        found.sort();
        assert_eq!(
            found,
            vec![
                root.0.join("postgresql@15/bin"),
                root.0.join("postgresql@16/bin"),
            ]
        );
        // Sem `*`, só confere se existe
        assert_eq!(
            expand_star(&root.0.join("libpq/bin")),
            vec![root.0.join("libpq/bin")]
        );
        assert!(expand_star(&root.0.join("nada/bin")).is_empty());
    }

    #[cfg(unix)]
    fn fake_tool(dir: &Path, name: &str, version_line: &str) {
        use std::os::unix::fs::PermissionsExt;
        let path = dir.join(name);
        std::fs::write(&path, format!("#!/bin/sh\necho '{version_line}'\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn inspects_a_folder_with_fake_tools() {
        let root = TempDir::new("inspect");
        fake_tool(&root.0, PG_DUMP, "pg_dump (PostgreSQL) 16.4 (Homebrew)");
        fake_tool(&root.0, PG_RESTORE, "pg_restore (PostgreSQL) 16.4");

        let install = inspect_dir(&root.0, PgToolsSource::Configured)
            .await
            .unwrap();
        assert_eq!(install.version, "16.4");
        assert_eq!(install.version_num, 160_004);
        assert!(install.pg_restore.is_some());
        assert_eq!(install.psql, None);

        // A pasta configurada vale mesmo havendo outras instalações
        let detection = detect(Some(root.0.to_str().unwrap())).await;
        assert_eq!(
            detection.selected.unwrap().dir,
            root.0.display().to_string()
        );
        assert_eq!(detection.configured_error, None);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn invalid_configured_folder_does_not_fall_back() {
        let root = TempDir::new("invalid");
        // Arquivo sem permissão de execução não conta
        std::fs::write(root.0.join(PG_DUMP), "não executável").unwrap();

        assert!(inspect_dir(&root.0, PgToolsSource::Configured)
            .await
            .is_err());
        let detection = detect(Some(root.0.to_str().unwrap())).await;
        assert!(detection.selected.is_none());
        assert!(detection
            .configured_error
            .unwrap()
            .contains("Não há pg_dump"));

        let missing = detect(Some("/caminho/que/nao/existe")).await;
        assert!(missing.configured_error.unwrap().contains("não existe"));
    }
}
