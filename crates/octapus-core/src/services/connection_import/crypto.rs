//! Decifrar as senhas que DBeaver e Beekeeper guardam em arquivo. Os dois
//! usam AES-CBC com chave fixa e pública (está no código-fonte deles) — não é
//! segredo nenhum, é só ofuscação; por isso dá para importar as senhas.

use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
use base64::Engine;
use sha2::{Digest, Sha256};

type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;
type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;

/// Chave do `credentials-config.json` do DBeaver Community.
const DBEAVER_KEY: [u8; 16] = [
    0xba, 0xbb, 0x4a, 0x9f, 0x77, 0x4a, 0xb8, 0x53, 0xc9, 0x6c, 0x2d, 0x65, 0x3d, 0xfe, 0x54, 0x4a,
];

/// Chave com que o Beekeeper cifra o arquivo `.key` (que guarda a chave de
/// cada instalação, usada nas senhas).
const BEEKEEPER_BOOTSTRAP_KEY: &str =
    "38782F413F442A472D4B6150645367566B59703373367639792442264529482B";

/// `credentials-config.json`: os 16 primeiros bytes são o IV, o resto é
/// AES-128-CBC com PKCS#7. Com senha mestra no projeto não decifra → `None`.
pub fn decrypt_dbeaver(data: &[u8]) -> Option<Vec<u8>> {
    if data.len() <= 16 {
        return None;
    }
    let (iv, ciphertext) = data.split_at(16);
    Aes128CbcDec::new_from_slices(&DBEAVER_KEY, iv)
        .ok()?
        .decrypt_padded_vec_mut::<Pkcs7>(ciphertext)
        .ok()
}

/// Formato do `simple-encryptor` (Node): `<hmac hex 64><iv hex 32><base64>`,
/// AES-256-CBC com chave = SHA-256 do texto da chave. O texto claro é JSON.
/// O HMAC não é conferido: arquivo corrompido falha ao decifrar ou no JSON.
fn decrypt_simple_encryptor(payload: &str, key: &str) -> Option<Vec<u8>> {
    // `get` em vez de índice: payload malformado com acento não pode dar panic
    let iv = hex_decode(payload.get(64..96)?)?;
    let ciphertext = base64::engine::general_purpose::STANDARD
        .decode(payload.get(96..)?.trim())
        .ok()?;
    let key = Sha256::digest(key.as_bytes());
    Aes256CbcDec::new_from_slices(&key, &iv)
        .ok()?
        .decrypt_padded_vec_mut::<Pkcs7>(&ciphertext)
        .ok()
}

/// A chave da instalação do Beekeeper, a partir do conteúdo do `.key`.
pub fn beekeeper_install_key(key_file: &str) -> Option<String> {
    let plain = decrypt_simple_encryptor(key_file.trim(), BEEKEEPER_BOOTSTRAP_KEY)?;
    let value: serde_json::Value = serde_json::from_slice(&plain).ok()?;
    value.get("encryptionKey")?.as_str().map(str::to_string)
}

/// Uma coluna cifrada do Beekeeper (a senha é uma string JSON).
pub fn beekeeper_decrypt_string(payload: &str, install_key: &str) -> Option<String> {
    let plain = decrypt_simple_encryptor(payload, install_key)?;
    serde_json::from_slice::<String>(&plain).ok()
}

fn hex_decode(hex: &str) -> Option<Vec<u8>> {
    if !hex.len().is_multiple_of(2) {
        return None;
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok())
        .collect()
}

#[cfg(test)]
pub mod test_support {
    //! Cifra no formato de cada app — os testes montam fixtures com elas.
    use aes::cipher::{BlockEncryptMut, KeyIvInit};
    use base64::Engine;
    use sha2::{Digest, Sha256};

    use super::*;

    type Aes128CbcEnc = cbc::Encryptor<aes::Aes128>;
    type Aes256CbcEnc = cbc::Encryptor<aes::Aes256>;

    pub fn encrypt_dbeaver(plain: &[u8]) -> Vec<u8> {
        let iv = [7u8; 16];
        let mut out = iv.to_vec();
        out.extend(
            Aes128CbcEnc::new_from_slices(&DBEAVER_KEY, &iv)
                .unwrap()
                .encrypt_padded_vec_mut::<Pkcs7>(plain),
        );
        out
    }

    pub fn encrypt_simple(plain: &[u8], key: &str) -> String {
        let iv = [3u8; 16];
        let key = Sha256::digest(key.as_bytes());
        let ciphertext = Aes256CbcEnc::new_from_slices(&key, &iv)
            .unwrap()
            .encrypt_padded_vec_mut::<Pkcs7>(plain);
        let iv_hex: String = iv.iter().map(|b| format!("{b:02x}")).collect();
        format!(
            "{}{iv_hex}{}",
            "0".repeat(64),
            base64::engine::general_purpose::STANDARD.encode(ciphertext)
        )
    }

    pub fn beekeeper_key_file(install_key: &str) -> String {
        let json = serde_json::json!({ "encryptionKey": install_key }).to_string();
        encrypt_simple(json.as_bytes(), BEEKEEPER_BOOTSTRAP_KEY)
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::*;
    use super::*;

    #[test]
    fn dbeaver_round_trip_and_garbage() {
        let plain = br##"{"postgres-1":{"#connection":{"user":"app","password":"s3cret"}}}"##;
        assert_eq!(decrypt_dbeaver(&encrypt_dbeaver(plain)).unwrap(), plain);
        assert!(decrypt_dbeaver(&[0u8; 8]).is_none());
        assert!(decrypt_dbeaver(&[1u8; 33]).is_none());
    }

    #[test]
    fn beekeeper_key_file_then_password() {
        let key_file = beekeeper_key_file("chave-da-instalacao");
        let install_key = beekeeper_install_key(&key_file).unwrap();
        assert_eq!(install_key, "chave-da-instalacao");

        let column = encrypt_simple(br#""hunter2""#, &install_key);
        assert_eq!(
            beekeeper_decrypt_string(&column, &install_key).as_deref(),
            Some("hunter2")
        );
        assert!(beekeeper_decrypt_string(&column, "outra-chave").is_none());
        // Payload curto ou com acento no meio não pode dar panic
        assert!(beekeeper_decrypt_string("abc", &install_key).is_none());
        assert!(beekeeper_decrypt_string(&"é".repeat(60), &install_key).is_none());
    }
}
