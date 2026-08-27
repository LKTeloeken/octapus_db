//! App-managed secrets vault — encryption at rest for server passwords.
//!
//! Replaces the OS keychain (see `secrets.rs`, kept only for migration).
//!
//! # Dois modos
//!
//! - **Sem senha mestre (padrão).** `vault.key` guarda 32 bytes crus, gerados na
//!   1ª execução e presos ao dispositivo. Zero prompts, comportamento idêntico
//!   em macOS/Windows/Linux. Protege contra o `app.db` ser copiado **sem** o
//!   `vault.key` — não contra quem tem os dois arquivos.
//! - **Com senha mestre (opt-in).** O mesmo DEK passa a ser guardado
//!   **embrulhado** por uma KEK derivada da senha via Argon2id. Nada no SQLite é
//!   re-criptografado: a chave que cifra os dados não muda, só o jeito de
//!   guardá-la. É o único modo que protege contra um processo rodando como o
//!   próprio usuário, porque o segredo passa a estar na cabeça dele.
//!
//! Ligar e desligar são reversíveis. Esquecer a senha **não é**: sem ela o DEK
//! é irrecuperável, e o único caminho é [`reset`], que descarta os segredos.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use parking_lot::RwLock;
use rand::RngCore;
use zeroize::Zeroize;

use crate::error::{Error, Result};

/// Envelope prefix + version, so plaintext (legacy) values are distinguishable
/// from ciphertext and the format can evolve.
const PREFIX: &str = "v1:";
const NONCE_LEN: usize = 12;
const DEK_LEN: usize = 32;

/// Cabeçalho do `vault.key` com senha mestre. O formato antigo (32 bytes crus)
/// não tem cabeçalho nenhum, então o tamanho já distingue os dois.
const MAGIC: &[u8; 8] = b"OCTAVLT2";
const SALT_LEN: usize = 16;
/// DEK + tag do GCM.
const WRAPPED_LEN: usize = DEK_LEN + 16;
const V2_LEN: usize = MAGIC.len() + SALT_LEN + 12 + NONCE_LEN + WRAPPED_LEN;

/// Custo do Argon2id. Gravado no arquivo para que subir esses números depois
/// não torne ilegíveis os cofres já criados.
const M_COST: u32 = 65_536; // 64 MiB
const T_COST: u32 = 3;
const P_COST: u32 = 1;

/// Chave de 32 bytes que se apaga no `drop` — o Rust libera sem sobrescrever.
#[derive(Clone)]
struct Dek([u8; DEK_LEN]);

impl Drop for Dek {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

/// `Debug` à mão: derivar imprimiria a chave-mestra do cofre inteiro.
impl fmt::Debug for Dek {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Dek(<redigido>)")
    }
}

static KEY: RwLock<Option<Dek>> = RwLock::new(None);
static KEY_PATH: OnceLock<PathBuf> = OnceLock::new();
static HAS_MASTER: AtomicBool = AtomicBool::new(false);

/// Situação do cofre logo após o boot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultState {
    /// Chave disponível: dá para cifrar e decifrar.
    Unlocked,
    /// Existe senha mestre e ela ainda não foi digitada nesta sessão.
    Locked,
    /// `vault.key` existe mas não é legível em nenhum dos formatos.
    Corrupt,
}

/// Carrega o `vault.key`, criando-o na primeira execução.
///
/// **Nunca falha o boot por causa do conteúdo do arquivo:** um cofre trancado ou
/// corrompido é um estado reportável, não um crash. O app precisa abrir para o
/// usuário conseguir digitar a senha — ou desistir e resetar.
pub fn init(app_data_dir: &Path) -> Result<VaultState> {
    let path = app_data_dir.join("vault.key");
    let _ = KEY_PATH.set(path.clone());

    if !path.exists() {
        let dek = random_dek();
        write_atomic(&path, &dek.0)?;
        *KEY.write() = Some(dek);
        HAS_MASTER.store(false, Ordering::Relaxed);
        return Ok(VaultState::Unlocked);
    }

    Ok(match load_key_file(&path)? {
        LoadedKey::Raw(dek) => {
            *KEY.write() = Some(dek);
            HAS_MASTER.store(false, Ordering::Relaxed);
            VaultState::Unlocked
        }
        LoadedKey::Wrapped => {
            HAS_MASTER.store(true, Ordering::Relaxed);
            VaultState::Locked
        }
        LoadedKey::Corrupt => VaultState::Corrupt,
    })
}

/// O que há no `vault.key`, sem tocar em estado global — é o que torna o ciclo
/// de vida do cofre testável com um arquivo de verdade.
enum LoadedKey {
    /// Formato antigo: 32 bytes crus, sem senha mestre.
    Raw(Dek),
    /// Formato v2: DEK embrulhado, precisa da senha mestre.
    Wrapped,
    /// Ilegível em qualquer formato conhecido.
    Corrupt,
}

fn load_key_file(path: &Path) -> Result<LoadedKey> {
    let bytes = std::fs::read(path)
        .map_err(|e| Error::Storage(format!("Failed to read vault key: {e}")))?;

    if let Ok(raw) = <[u8; DEK_LEN]>::try_from(bytes.as_slice()) {
        return Ok(LoadedKey::Raw(Dek(raw)));
    }

    if parse_v2(&bytes).is_some() {
        return Ok(LoadedKey::Wrapped);
    }

    Ok(LoadedKey::Corrupt)
}

pub fn state() -> VaultState {
    if KEY.read().is_some() {
        VaultState::Unlocked
    } else if HAS_MASTER.load(Ordering::Relaxed) {
        VaultState::Locked
    } else {
        VaultState::Corrupt
    }
}

pub fn has_master_password() -> bool {
    HAS_MASTER.load(Ordering::Relaxed)
}

pub fn is_locked() -> bool {
    KEY.read().is_none()
}

/// Destrava com a senha mestre. O GCM autentica: senha errada não decifra.
pub fn unlock(password: &str) -> Result<()> {
    let bytes = read_key_file()?;
    let v2 = parse_v2(&bytes).ok_or(Error::VaultCorrupt)?;
    let dek = v2.unwrap_dek(password)?;

    *KEY.write() = Some(dek);
    Ok(())
}

/// Devolve o cofre ao estado trancado, apagando o DEK da memória.
pub fn lock() {
    if has_master_password() {
        // O `Drop` do `Dek` zera os bytes.
        *KEY.write() = None;
    }
}

/// Passa a guardar o DEK embrulhado pela senha. **Nenhum dado é
/// re-criptografado** — o DEK é o mesmo, só muda como o arquivo o guarda.
pub fn enable_master_password(password: &str) -> Result<()> {
    if password.is_empty() {
        return Err(Error::InvalidState("A senha mestre não pode ser vazia".into()));
    }

    let dek = current_dek()?;
    let wrapped = wrap_dek(&dek, password)?;

    replace_key_file(&key_path()?, &wrapped, |bytes| {
        parse_v2(bytes)
            .ok_or(Error::VaultCorrupt)?
            .unwrap_dek(password)
            .map(|_| ())
    })?;

    HAS_MASTER.store(true, Ordering::Relaxed);
    Ok(())
}

/// Volta a guardar a chave crua. Exige o cofre destravado.
pub fn disable_master_password() -> Result<()> {
    let dek = current_dek()?;

    replace_key_file(&key_path()?, &dek.0, |bytes| {
        <[u8; DEK_LEN]>::try_from(bytes)
            .map(|_| ())
            .map_err(|_| Error::VaultCorrupt)
    })?;

    HAS_MASTER.store(false, Ordering::Relaxed);
    Ok(())
}

/// Descarta o cofre e começa do zero com uma chave nova.
///
/// **Destrutivo:** tudo que estava cifrado com a chave antiga vira lixo. Quem
/// chama é responsável por limpar os segredos guardados no SQLite.
pub fn reset() -> Result<()> {
    let path = key_path()?;
    let dek = random_dek();

    write_atomic(&path, &dek.0)?;
    let _ = std::fs::remove_file(backup_path(&path));

    *KEY.write() = Some(dek);
    HAS_MASTER.store(false, Ordering::Relaxed);
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Cifra dos segredos (inalterada — sempre usa o DEK, com ou sem senha mestre)
// ─────────────────────────────────────────────────────────────────────────────

fn cipher() -> Result<Aes256Gcm> {
    let guard = KEY.read();
    let dek = guard.as_ref().ok_or(Error::VaultLocked)?;
    Ok(Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&dek.0)))
}

/// Encrypt a secret into a self-describing envelope (`v1:<hex(nonce||ct)>`).
pub fn encrypt(plaintext: &str) -> Result<String> {
    let cipher = cipher()?;

    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);

    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce_bytes), plaintext.as_bytes())
        .map_err(|_| Error::Storage("Failed to encrypt secret".into()))?;

    let mut blob = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    blob.extend_from_slice(&nonce_bytes);
    blob.extend_from_slice(&ciphertext);

    Ok(format!("{PREFIX}{}", hex::encode(blob)))
}

/// Decrypt an envelope produced by [`encrypt`]. Returns `None` if the input is
/// not an envelope, the vault is locked, or the key/ciphertext doesn't match.
pub fn decrypt(envelope: &str) -> Option<String> {
    let blob = hex::decode(envelope.strip_prefix(PREFIX)?).ok()?;
    if blob.len() <= NONCE_LEN {
        return None;
    }

    let (nonce_bytes, ciphertext) = blob.split_at(NONCE_LEN);
    let plaintext = cipher()
        .ok()?
        .decrypt(Nonce::from_slice(nonce_bytes), ciphertext)
        .ok()?;

    String::from_utf8(plaintext).ok()
}

/// Whether a stored value is a vault envelope (vs. legacy plaintext).
pub fn is_envelope(value: &str) -> bool {
    value.starts_with(PREFIX)
}

// ─────────────────────────────────────────────────────────────────────────────
// Formato v2 (DEK embrulhado)
// ─────────────────────────────────────────────────────────────────────────────

struct WrappedKey {
    salt: [u8; SALT_LEN],
    m_cost: u32,
    t_cost: u32,
    p_cost: u32,
    nonce: [u8; NONCE_LEN],
    wrapped: Vec<u8>,
}

impl WrappedKey {
    fn unwrap_dek(&self, password: &str) -> Result<Dek> {
        let mut kek = derive_kek(password, &self.salt, self.m_cost, self.t_cost, self.p_cost)?;

        let opened = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&kek))
            .decrypt(Nonce::from_slice(&self.nonce), self.wrapped.as_slice());
        kek.zeroize();

        // Tag do GCM inválida = senha errada (ou arquivo adulterado).
        let mut plain = opened.map_err(|_| Error::WrongMasterPassword)?;
        let dek = <[u8; DEK_LEN]>::try_from(plain.as_slice()).map_err(|_| Error::VaultCorrupt);
        plain.zeroize();

        Ok(Dek(dek?))
    }
}

fn parse_v2(bytes: &[u8]) -> Option<WrappedKey> {
    if bytes.len() != V2_LEN || &bytes[..MAGIC.len()] != MAGIC {
        return None;
    }

    let mut at = MAGIC.len();
    let mut take = |n: usize| {
        let slice = &bytes[at..at + n];
        at += n;
        slice
    };

    let salt = <[u8; SALT_LEN]>::try_from(take(SALT_LEN)).ok()?;
    let m_cost = u32::from_le_bytes(take(4).try_into().ok()?);
    let t_cost = u32::from_le_bytes(take(4).try_into().ok()?);
    let p_cost = u32::from_le_bytes(take(4).try_into().ok()?);
    let nonce = <[u8; NONCE_LEN]>::try_from(take(NONCE_LEN)).ok()?;
    let wrapped = take(WRAPPED_LEN).to_vec();

    Some(WrappedKey {
        salt,
        m_cost,
        t_cost,
        p_cost,
        nonce,
        wrapped,
    })
}

fn wrap_dek(dek: &Dek, password: &str) -> Result<Vec<u8>> {
    let mut salt = [0u8; SALT_LEN];
    let mut nonce = [0u8; NONCE_LEN];
    rand::rngs::OsRng.fill_bytes(&mut salt);
    rand::rngs::OsRng.fill_bytes(&mut nonce);

    let mut kek = derive_kek(password, &salt, M_COST, T_COST, P_COST)?;
    let wrapped = Aes256Gcm::new(Key::<Aes256Gcm>::from_slice(&kek))
        .encrypt(Nonce::from_slice(&nonce), dek.0.as_slice())
        .map_err(|_| Error::Storage("Falha ao embrulhar a chave do cofre".into()));
    kek.zeroize();

    let mut out = Vec::with_capacity(V2_LEN);
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&salt);
    out.extend_from_slice(&M_COST.to_le_bytes());
    out.extend_from_slice(&T_COST.to_le_bytes());
    out.extend_from_slice(&P_COST.to_le_bytes());
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&wrapped?);

    debug_assert_eq!(out.len(), V2_LEN);
    Ok(out)
}

fn derive_kek(
    password: &str,
    salt: &[u8],
    m_cost: u32,
    t_cost: u32,
    p_cost: u32,
) -> Result<[u8; DEK_LEN]> {
    let params = Params::new(m_cost, t_cost, p_cost, Some(DEK_LEN))
        .map_err(|_| Error::Storage("Parâmetros de Argon2 inválidos no vault.key".into()))?;

    let mut kek = [0u8; DEK_LEN];
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(password.as_bytes(), salt, &mut kek)
        .map_err(|_| Error::Storage("Falha ao derivar a chave da senha mestre".into()))?;

    Ok(kek)
}

// ─────────────────────────────────────────────────────────────────────────────
// Arquivo
// ─────────────────────────────────────────────────────────────────────────────

fn key_path() -> Result<PathBuf> {
    KEY_PATH
        .get()
        .cloned()
        .ok_or_else(|| Error::Storage("Vault not initialized".into()))
}

fn read_key_file() -> Result<Vec<u8>> {
    std::fs::read(key_path()?)
        .map_err(|e| Error::Storage(format!("Failed to read vault key: {e}")))
}

fn current_dek() -> Result<Dek> {
    KEY.read().clone().ok_or(Error::VaultLocked)
}

fn random_dek() -> Dek {
    let mut key = [0u8; DEK_LEN];
    rand::rngs::OsRng.fill_bytes(&mut key);
    Dek(key)
}

fn backup_path(path: &Path) -> PathBuf {
    path.with_extension("bak")
}

/// Troca o `vault.key` por um conteúdo novo, com rede de segurança.
///
/// Este é o ponto onde um erro custa **todas as senhas guardadas**, então:
/// guarda um backup, escreve de forma atômica, relê o que foi gravado e só
/// então apaga o backup. Se a verificação falhar, restaura e devolve erro.
fn replace_key_file(
    path: &Path,
    bytes: &[u8],
    verify: impl Fn(&[u8]) -> Result<()>,
) -> Result<()> {
    let backup = backup_path(path);

    std::fs::copy(path, &backup)
        .map_err(|e| Error::Storage(format!("Falha ao salvar backup do vault.key: {e}")))?;

    let restore = |err: Error| -> Error {
        let _ = std::fs::rename(&backup, path);
        err
    };

    write_atomic(path, bytes).map_err(restore)?;

    let written = std::fs::read(path)
        .map_err(|e| restore(Error::Storage(format!("Falha ao reler o vault.key: {e}"))))?;

    verify(&written).map_err(restore)?;

    let _ = std::fs::remove_file(&backup);
    Ok(())
}

/// Grava num temporário, força para o disco e só então renomeia por cima —
/// um crash no meio deixa o `vault.key` antigo intacto em vez de um híbrido.
fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;

    let tmp = path.with_extension("new");

    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    let mut file = options
        .open(&tmp)
        .map_err(|e| Error::Storage(format!("Failed to create vault key: {e}")))?;
    file.write_all(bytes)
        .map_err(|e| Error::Storage(format!("Failed to write vault key: {e}")))?;
    file.sync_all()
        .map_err(|e| Error::Storage(format!("Failed to flush vault key: {e}")))?;
    drop(file);

    std::fs::rename(&tmp, path)
        .map_err(|e| Error::Storage(format!("Failed to replace vault key: {e}")))?;

    Ok(())
}

/// Seed a deterministic key for unit tests (no key file, no OS access).
#[cfg(test)]
pub(crate) fn init_for_tests() {
    let mut guard = KEY.write();
    if guard.is_none() {
        *guard = Some(Dek([7u8; DEK_LEN]));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SENHA: &str = "uma senha mestre qualquer";

    fn sample_dek() -> Dek {
        Dek([42u8; DEK_LEN])
    }

    /// Diretório temporário próprio, para não esbarrar em teste paralelo.
    fn temp_file(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("octapus-vault-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn wrap_and_unwrap_round_trip() {
        let dek = sample_dek();
        let file = wrap_dek(&dek, SENHA).unwrap();

        assert_eq!(file.len(), V2_LEN);
        assert_eq!(&file[..MAGIC.len()], MAGIC);
        // O DEK não pode aparecer em claro dentro do arquivo.
        assert!(
            !file.windows(DEK_LEN).any(|w| w == dek.0),
            "DEK gravado em claro no vault.key"
        );

        let recovered = parse_v2(&file).unwrap().unwrap_dek(SENHA).unwrap();
        assert_eq!(recovered.0, dek.0, "o DEK precisa sobreviver intacto");
    }

    #[test]
    fn wrong_password_is_rejected() {
        let file = wrap_dek(&sample_dek(), SENHA).unwrap();

        let err = parse_v2(&file).unwrap().unwrap_dek("senha errada").unwrap_err();
        assert_eq!(err.code(), "WRONG_MASTER_PASSWORD");
    }

    #[test]
    fn tampered_file_is_rejected() {
        let mut file = wrap_dek(&sample_dek(), SENHA).unwrap();

        // Vira um bit do DEK embrulhado: a tag do GCM tem que acusar.
        let last = file.len() - 1;
        file[last] ^= 0x01;

        let err = parse_v2(&file).unwrap().unwrap_dek(SENHA).unwrap_err();
        assert_eq!(err.code(), "WRONG_MASTER_PASSWORD");
    }

    #[test]
    fn argon_params_survive_the_round_trip() {
        let file = wrap_dek(&sample_dek(), SENHA).unwrap();
        let parsed = parse_v2(&file).unwrap();

        // Gravados no arquivo para que subir o custo depois não quebre cofres
        // que já existem.
        assert_eq!((parsed.m_cost, parsed.t_cost, parsed.p_cost), (M_COST, T_COST, P_COST));
    }

    #[test]
    fn raw_key_is_not_mistaken_for_a_wrapped_one() {
        // O formato antigo (32 bytes crus) não pode ser lido como v2, senão o
        // upgrade acharia que existe senha mestre onde não existe.
        assert!(parse_v2(&[0u8; DEK_LEN]).is_none());
        assert!(parse_v2(b"").is_none());
        assert!(parse_v2(&[0u8; V2_LEN]).is_none(), "faltou checar o magic");
    }

    /// O teste mais importante do arquivo: se a troca do `vault.key` falhar no
    /// meio, o cofre antigo precisa continuar de pé. Um erro aqui custa **todas
    /// as senhas guardadas** do usuário.
    #[test]
    fn a_failed_replacement_restores_the_original_key() {
        let path = temp_file("restore.key");
        std::fs::write(&path, [7u8; DEK_LEN]).unwrap();

        let err = replace_key_file(&path, &[9u8; DEK_LEN], |_| {
            Err(Error::VaultCorrupt) // verificação sempre falha
        })
        .unwrap_err();

        assert_eq!(err.code(), "VAULT_CORRUPT");
        assert_eq!(
            std::fs::read(&path).unwrap(),
            vec![7u8; DEK_LEN],
            "o vault.key original foi perdido numa troca que falhou"
        );
    }

    #[test]
    fn a_successful_replacement_leaves_no_leftovers() {
        let path = temp_file("clean.key");
        std::fs::write(&path, [7u8; DEK_LEN]).unwrap();

        replace_key_file(&path, &[9u8; DEK_LEN], |_| Ok(())).unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), vec![9u8; DEK_LEN]);
        assert!(!backup_path(&path).exists(), "backup ficou para trás");
        assert!(!path.with_extension("new").exists(), "temporário ficou para trás");
    }

    /// O ciclo de vida inteiro contra um arquivo de verdade. A afirmação
    /// central do key wrapping é que **o DEK nunca muda** — é por isso que
    /// ativar a senha mestre não precisa recriptografar nada no SQLite.
    #[test]
    fn the_dek_survives_the_whole_master_password_lifecycle() {
        let path = temp_file("lifecycle.key");
        let dek = sample_dek();
        write_atomic(&path, &dek.0).unwrap();

        let verify_wrapped = |bytes: &[u8]| {
            parse_v2(bytes)
                .ok_or(Error::VaultCorrupt)?
                .unwrap_dek(SENHA)
                .map(|_| ())
        };
        let verify_raw = |bytes: &[u8]| {
            <[u8; DEK_LEN]>::try_from(bytes)
                .map(|_| ())
                .map_err(|_| Error::VaultCorrupt)
        };

        // 1. Sem senha mestre: chave crua, cofre aberto direto.
        let LoadedKey::Raw(loaded) = load_key_file(&path).unwrap() else {
            panic!("esperava chave crua");
        };
        assert_eq!(loaded.0, dek.0);

        // 2. Ativar a senha mestre.
        let wrapped = wrap_dek(&dek, SENHA).unwrap();
        replace_key_file(&path, &wrapped, verify_wrapped).unwrap();

        // 3. Boot seguinte enxerga o cofre trancado.
        assert!(matches!(load_key_file(&path).unwrap(), LoadedKey::Wrapped));

        // 4. Destravar devolve o **mesmo** DEK.
        let bytes = std::fs::read(&path).unwrap();
        let unlocked = parse_v2(&bytes).unwrap().unwrap_dek(SENHA).unwrap();
        assert_eq!(
            unlocked.0, dek.0,
            "o DEK mudou — todo segredo já guardado teria virado lixo"
        );

        // 5. Desativar volta ao formato antigo, ainda com o mesmo DEK.
        replace_key_file(&path, &unlocked.0, verify_raw).unwrap();
        let LoadedKey::Raw(back) = load_key_file(&path).unwrap() else {
            panic!("esperava voltar para chave crua");
        };
        assert_eq!(back.0, dek.0);
    }

    #[test]
    fn a_corrupt_key_file_is_reported_not_panicked() {
        let path = temp_file("corrupt.key");
        std::fs::write(&path, b"isto nao e uma chave").unwrap();

        assert!(matches!(load_key_file(&path).unwrap(), LoadedKey::Corrupt));
    }

    #[cfg(unix)]
    #[test]
    fn key_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;

        let path = temp_file("perms.key");
        write_atomic(&path, &[1u8; DEK_LEN]).unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "vault.key precisa ser 0600");
    }
}
