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
/// - Linux: `~/.tableplus/settings/connections.json` (e onde ficaria se o app
///   seguir o XDG, como pedem nas issues dele).
pub fn tableplus_files() -> Vec<PathBuf> {
    let plist_in = |base: Option<PathBuf>| -> Vec<PathBuf> {
        let Some(base) = base else { return Vec::new() };
        ["com.tinyapp.TablePlus", "com.tinyapp.TablePlus-setapp"]
            .iter()
            .map(|bundle| base.join(bundle).join("Data/Connections.plist"))
            .collect()
    };
    if cfg!(target_os = "macos") {
        plist_in(dirs::data_dir())
    } else if cfg!(target_os = "windows") {
        plist_in(dirs::data_local_dir())
    } else {
        [
            dirs::home_dir().map(|home| home.join(".tableplus/settings/connections.json")),
            dirs::data_dir().map(|dir| dir.join("tableplus/connections.json")),
            dirs::config_dir().map(|dir| dir.join("tableplus/connections.json")),
        ]
        .into_iter()
        .flatten()
        .collect()
    }
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
