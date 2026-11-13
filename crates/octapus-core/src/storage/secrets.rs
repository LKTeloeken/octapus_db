//! Legacy OS-keychain access (macOS Keychain / Windows Credential Manager /
//! Linux Secret Service). **Migration-only:** new secrets go to the in-app
//! `vault`; this module is kept solely so existing keychain-stored passwords
//! can be read once and migrated into the vault (see
//! `repositories::servers::decrypt_password`). Remove once the install base has
//! migrated — at that point the `keyring` dependency can be dropped too.

use keyring::Entry;

const SERVICE: &str = "octapus_db";

fn entry(server_id: i64) -> keyring::Result<Entry> {
    Entry::new(SERVICE, &format!("server-{server_id}"))
}

/// Read a legacy keychain password so it can be migrated into the vault.
pub fn get_password(server_id: i64) -> Option<String> {
    entry(server_id).and_then(|e| e.get_password()).ok()
}

/// Best-effort removal (no-op if the entry doesn't exist).
pub fn delete_password(server_id: i64) {
    if let Ok(e) = entry(server_id) {
        let _ = e.delete_credential();
    }
}
