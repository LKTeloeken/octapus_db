//! Ler senhas que outro app guardou no Keychain do macOS (TablePlus). Cada
//! leitura faz o sistema pedir permissão ao usuário — por isso só acontece no
//! commit, e só das conexões escolhidas.

/// Resultado de uma leitura.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeychainRead {
    Found(String),
    NotFound,
    /// Permissão negada (ou item ilegível): quem chama para de pedir
    Denied,
}

#[cfg(target_os = "macos")]
pub fn read_password(service: &str, account: &str) -> KeychainRead {
    let Ok(entry) = keyring::Entry::new(service, account) else {
        return KeychainRead::NotFound;
    };
    match entry.get_password() {
        Ok(password) => KeychainRead::Found(password),
        Err(keyring::Error::NoEntry) => KeychainRead::NotFound,
        Err(_) => KeychainRead::Denied,
    }
}

/// Fora do macOS o TablePlus não usa um cofre que dê para ler.
#[cfg(not(target_os = "macos"))]
pub fn read_password(_service: &str, _account: &str) -> KeychainRead {
    KeychainRead::NotFound
}
