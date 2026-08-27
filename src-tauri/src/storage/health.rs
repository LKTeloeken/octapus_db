//! Saúde do cofre e manutenção do SQLite.
//!
//! **Canário:** um valor conhecido, cifrado com a mesma chave das senhas e
//! guardado na tabela `meta`. Ele responde no boot a pergunta que antes só
//! aparecia como "authentication failed" na cara do usuário: *a chave que está
//! em `vault.key` é a mesma que cifrou o que está no banco?*
//!
//! Sem ele, um `vault.key` trocado/corrompido fazia toda senha decifrar para
//! `None` em silêncio, e o erro só surgia lá na frente, disfarçado de credencial
//! errada do Postgres.

use rusqlite::Connection;

use crate::error::Result;
use crate::storage::repositories::meta;
use crate::storage::vault;

const CANARY_KEY: &str = "vault_canary";
const CANARY_PLAINTEXT: &str = "octapus-vault-v1";
const PENDING_VACUUM_KEY: &str = "pending_vacuum";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultHealth {
    /// A chave abre o que está guardado (ou não há nada guardado ainda).
    Ok,
    /// Existe canário, mas a chave atual não o decifra: `vault.key` foi trocado,
    /// corrompido ou perdido. As senhas guardadas são irrecuperáveis.
    KeyMismatch,
}

/// Valida o canário; se ainda não existir, cria um com a chave atual.
///
/// Chamada uma vez no startup. Nunca falha o boot — um cofre quebrado ainda
/// deixa o usuário abrir o app, ver os servidores e redigitar as senhas.
pub fn check_or_seed(conn: &Connection) -> VaultHealth {
    // Cofre trancado não é cofre quebrado: sem o DEK não dá para julgar, e
    // semear agora gravaria um canário com uma chave que não existe.
    if vault::is_locked() {
        return VaultHealth::Ok;
    }

    match meta::get(conn, CANARY_KEY) {
        Ok(Some(stored)) => match vault::decrypt(&stored) {
            Some(plain) if plain == CANARY_PLAINTEXT => VaultHealth::Ok,
            _ => VaultHealth::KeyMismatch,
        },
        Ok(None) => {
            // Primeira execução com canário (instalação nova ou já existente):
            // sela a chave atual como a boa.
            if let Ok(envelope) = vault::encrypt(CANARY_PLAINTEXT) {
                let _ = meta::set(conn, CANARY_KEY, &envelope);
            }
            VaultHealth::Ok
        }
        // Falha de leitura do SQLite não é problema de cofre.
        Err(_) => VaultHealth::Ok,
    }
}

/// Apaga o canário. Usado no reset destrutivo: a chave nova precisa selar um
/// canário novo, senão o antigo (da chave velha) acusaria incompatibilidade.
pub fn clear_canary(conn: &Connection) -> Result<()> {
    meta::delete(conn, CANARY_KEY)
}

/// Marca que um segredo legado em texto puro foi migrado para o cofre, então
/// sobrou plaintext em páginas livres do `app.db` que só o `VACUUM` remove.
pub fn mark_pending_vacuum(conn: &Connection) {
    let _ = meta::set(conn, PENDING_VACUUM_KEY, "1");
}

/// Roda o `VACUUM` pendente, se houver, e limpa a flag.
///
/// O SQLite não sobrescreve páginas liberadas: depois de migrar uma senha (ou
/// URI) que estava em texto puro, o valor antigo continua legível no espaço
/// livre do arquivo. O `VACUUM` reescreve o banco e leva esse resíduo junto.
/// Roda no startup seguinte à migração, e não a cada boot — é caro à toa.
pub fn run_pending_vacuum(conn: &Connection) -> Result<()> {
    if meta::get(conn, PENDING_VACUUM_KEY)?.is_none() {
        return Ok(());
    }

    // Um VACUUM que falha não pode impedir o app de abrir; a flag fica de pé
    // para a próxima tentativa.
    if conn.execute_batch("VACUUM;").is_ok() {
        meta::delete(conn, PENDING_VACUUM_KEY)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::database::init_storage;

    #[test]
    fn canary_is_seeded_then_accepted() {
        vault::init_for_tests();
        let conn = init_storage(":memory:").unwrap();

        // Primeira passada sela a chave; as seguintes reconhecem.
        assert_eq!(check_or_seed(&conn), VaultHealth::Ok);
        assert!(meta::get(&conn, CANARY_KEY).unwrap().is_some());
        assert_eq!(check_or_seed(&conn), VaultHealth::Ok);
    }

    #[test]
    fn canary_detects_a_foreign_key() {
        vault::init_for_tests();
        let conn = init_storage(":memory:").unwrap();

        // Canário cifrado com outra chave (simula `vault.key` trocado).
        meta::set(&conn, CANARY_KEY, "v1:deadbeefdeadbeefdeadbeefdeadbeef").unwrap();

        assert_eq!(check_or_seed(&conn), VaultHealth::KeyMismatch);
    }

    #[test]
    fn vacuum_runs_once_and_clears_the_flag() {
        vault::init_for_tests();
        let conn = init_storage(":memory:").unwrap();

        run_pending_vacuum(&conn).unwrap(); // no-op sem flag

        mark_pending_vacuum(&conn);
        assert!(meta::get(&conn, PENDING_VACUUM_KEY).unwrap().is_some());

        run_pending_vacuum(&conn).unwrap();
        assert!(meta::get(&conn, PENDING_VACUUM_KEY).unwrap().is_none());
    }
}
