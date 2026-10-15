//! Onde cada app guarda as conexões, por sistema. `None` quando o sistema
//! não tem a pasta (ou o app não existe nele).

use std::path::PathBuf;

/// `~/Library/DBeaverData` no macOS; `<dados>/DBeaverData` nos outros
/// (`~/.local/share`, `%APPDATA%`).
pub fn dbeaver_root() -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        dirs::home_dir().map(|home| home.join("Library/DBeaverData"))
    } else {
        dirs::data_dir().map(|dir| dir.join("DBeaverData"))
    }
}

/// Onde o TablePlus guarda as conexões, em ordem de preferência:
/// - macOS: `Connections.plist` na pasta `Data` (versão normal e a do Setapp);
/// - Windows: o mesmo plist, no `%LOCALAPPDATA%`;
/// - Linux: `~/.tableplus/settings/connections.json` e, para instalações que
///   seguem o XDG (Nix, Flatpak…), o mesmo arquivo sob `XDG_CONFIG_HOME`,
///   `XDG_DATA_HOME` e `XDG_STATE_HOME` (o `dirs` já lê essas variáveis).
pub fn tableplus_files() -> Vec<PathBuf> {
    let plist_in = |base: Option<PathBuf>| -> Vec<PathBuf> {
        let Some(base) = base else { return Vec::new() };
        ["com.tinyapp.TablePlus", "com.tinyapp.TablePlus-setapp"]
            .iter()
            .map(|bundle| base.join(bundle).join("Data/Connections.plist"))
            .collect()
    };
    if cfg!(target_os = "macos") {
        return plist_in(dirs::data_dir());
    }
    if cfg!(target_os = "windows") {
        return plist_in(dirs::data_local_dir());
    }

    linux_tableplus_files(
        dirs::home_dir(),
        [dirs::config_dir(), dirs::data_dir(), dirs::state_dir()],
    )
}

/// Os candidatos do Linux: a pasta padrão do app na home e, em cada base XDG,
/// o mesmo arquivo com e sem o ponto (`.tableplus` ou `tableplus`).
fn linux_tableplus_files(home: Option<PathBuf>, xdg_bases: [Option<PathBuf>; 3]) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = home
        .map(|home| home.join(".tableplus/settings/connections.json"))
        .into_iter()
        .collect();
    for base in xdg_bases.into_iter().flatten() {
        for relative in [
            ".tableplus/settings/connections.json",
            "tableplus/settings/connections.json",
            "tableplus/connections.json",
        ] {
            let file = base.join(relative);
            if !files.contains(&file) {
                files.push(file);
            }
        }
    }
    files
}

/// Banco de configuração do pgAdmin 4 (modo desktop).
pub fn pgadmin_db() -> Option<PathBuf> {
    if cfg!(target_os = "windows") {
        dirs::config_dir().map(|dir| dir.join("pgAdmin/pgadmin4.db"))
    } else {
        dirs::home_dir().map(|home| home.join(".pgadmin/pgadmin4.db"))
    }
}

/// Pasta de dados do Beekeeper (userData do Electron).
pub fn beekeeper_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("beekeeper-studio"))
}

/// Raiz das configurações da JetBrains (tem uma pasta `DataGrip20xx.y` por versão).
pub fn jetbrains_root() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("JetBrains"))
}

/// `~/.pgpass` (ou `%APPDATA%\postgresql\pgpass.conf`), respeitando `PGPASSFILE`.
pub fn pgpass_file() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("PGPASSFILE") {
        return Some(PathBuf::from(path));
    }
    if cfg!(target_os = "windows") {
        dirs::config_dir().map(|dir| dir.join("postgresql/pgpass.conf"))
    } else {
        dirs::home_dir().map(|home| home.join(".pgpass"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_tableplus_candidates_cover_home_and_xdg() {
        let files = linux_tableplus_files(
            Some("/home/voce".into()),
            [
                Some("/home/voce/.config".into()),
                Some("/home/voce/.local/share".into()),
                Some("/home/voce/.local/state".into()),
            ],
        );
        assert_eq!(
            files[0],
            PathBuf::from("/home/voce/.tableplus/settings/connections.json")
        );
        for expected in [
            "/home/voce/.config/.tableplus/settings/connections.json",
            "/home/voce/.local/state/.tableplus/settings/connections.json",
            "/home/voce/.local/share/tableplus/settings/connections.json",
        ] {
            assert!(files.contains(&PathBuf::from(expected)), "{expected}");
        }
        // Sem home nem XDG (ambiente mínimo) não quebra
        assert!(linux_tableplus_files(None, [None, None, None]).is_empty());
    }
}
