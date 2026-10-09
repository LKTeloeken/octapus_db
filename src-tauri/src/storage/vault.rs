//! App-managed secrets vault — encryption at rest for server passwords.
//!
//! Replaces the OS keychain (see `secrets.rs`, kept only for migration). The
//! key is **device-bound**: a random 256-bit key generated on first run and
//! stored in `vault.key` (mode 0600) next to the SQLite database. This gives
//! zero prompts and identical behaviour across macOS/Windows/Linux.
//!
//! Trade-off: this is encryption *at rest*. It protects against the `app.db`
//! being copied/backed-up/synced without the key file, but not against a local
//! attacker who can read both files. For stronger protection a master password
//! would be needed (not implemented).

use std::path::Path;
use std::sync::OnceLock;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use rand::RngCore;

use crate::error::{Error, Result};

/// Envelope prefix + version, so plaintext (legacy) values are distinguishable
/// from ciphertext and the format can evolve.
const PREFIX: &str = "v1:";
const NONCE_LEN: usize = 12;

static KEY: OnceLock<[u8; 32]> = OnceLock::new();

/// Load the device key from `<app_data_dir>/vault.key`, generating it on first
/// run. Call once at startup before any encrypt/decrypt.
pub fn init(app_data_dir: &Path) -> Result<()> {
    let path = app_data_dir.join("vault.key");

    let key = if path.exists() {
        let bytes = std::fs::read(&path)
            .map_err(|e| Error::Storage(format!("Failed to read vault key: {e}")))?;
        <[u8; 32]>::try_from(bytes.as_slice())
            .map_err(|_| Error::Storage("Vault key file is corrupt".into()))?
    } else {
        let mut key = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut key);
        write_key(&path, &key)?;
        key
    };

    let _ = KEY.set(key);
    Ok(())
}

/// Write the key with owner-only permissions (0600 on Unix; on Windows the file
/// inherits the per-user profile ACL).
fn write_key(path: &Path, key: &[u8; 32]) -> Result<()> {
    use std::io::Write;

    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    let mut file = options
        .open(path)
        .map_err(|e| Error::Storage(format!("Failed to create vault key: {e}")))?;
    file.write_all(key)
        .map_err(|e| Error::Storage(format!("Failed to write vault key: {e}")))?;
    Ok(())
}

fn cipher() -> Result<Aes256Gcm> {
    let key = KEY
        .get()
        .ok_or_else(|| Error::Storage("Vault not initialized".into()))?;
    Ok(Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(key)))
}

/// Encrypt raw bytes with the device key: `nonce || ciphertext` (no text
/// envelope). Used for binary blobs such as the cached metadata catalog.
pub fn seal(plaintext: &[u8]) -> Result<Vec<u8>> {
    let cipher = cipher()?;

    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);

    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce_bytes), plaintext)
        .map_err(|_| Error::Storage("Failed to encrypt secret".into()))?;

    let mut blob = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    blob.extend_from_slice(&nonce_bytes);
    blob.extend_from_slice(&ciphertext);
    Ok(blob)
}

/// Decrypt bytes produced by [`seal`]. Returns `None` if the key or the
/// ciphertext doesn't match (GCM authenticates, so tampering is detected).
pub fn open(blob: &[u8]) -> Option<Vec<u8>> {
    if blob.len() <= NONCE_LEN {
        return None;
    }

    let (nonce_bytes, ciphertext) = blob.split_at(NONCE_LEN);
    cipher()
        .ok()?
        .decrypt(Nonce::from_slice(nonce_bytes), ciphertext)
        .ok()
}

/// Encrypt a secret into a self-describing envelope (`v1:<hex(nonce||ct)>`).
pub fn encrypt(plaintext: &str) -> Result<String> {
    Ok(format!("{PREFIX}{}", hex::encode(seal(plaintext.as_bytes())?)))
}

/// Decrypt an envelope produced by [`encrypt`]. Returns `None` if the input is
/// not an envelope or the key/ciphertext doesn't match.
pub fn decrypt(envelope: &str) -> Option<String> {
    let blob = hex::decode(envelope.strip_prefix(PREFIX)?).ok()?;
    String::from_utf8(open(&blob)?).ok()
}

/// Whether a stored value is a vault envelope (vs. legacy plaintext).
pub fn is_envelope(value: &str) -> bool {
    value.starts_with(PREFIX)
}

/// Seed a deterministic key for unit tests (no key file, no OS access).
#[cfg(test)]
pub(crate) fn init_for_tests() {
    let _ = KEY.set([7u8; 32]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_and_raw_blobs_round_trip() {
        init_for_tests();

        let envelope = encrypt("s3cret").unwrap();
        assert!(is_envelope(&envelope));
        assert_eq!(decrypt(&envelope).as_deref(), Some("s3cret"));

        let blob = seal(&[1, 2, 3, 0, 255]).unwrap();
        assert_eq!(open(&blob).unwrap(), [1, 2, 3, 0, 255]);
    }

    #[test]
    fn tampered_or_short_blobs_do_not_open() {
        init_for_tests();

        let mut blob = seal(b"catalog").unwrap();
        let last = blob.len() - 1;
        blob[last] ^= 0x01;
        assert!(open(&blob).is_none());
        assert!(open(&[0u8; NONCE_LEN]).is_none());
        assert!(decrypt("plaintext").is_none());
    }
}
