//! O `app.db`: o SQLite local com os servidores, as preferências e a sessão.
//!
//! O app e o terminal (`octapus`) abrem o mesmo arquivo, às vezes ao mesmo
//! tempo e em versões diferentes. Por isso:
//! - o schema é criado e migrado numa transação `IMMEDIATE`: dois processos
//!   partindo juntos fazem fila, e o segundo já encontra tudo pronto;
//! - quem espera a vez espera até [`BUSY_TIMEOUT`], em vez de falhar na hora
//!   (inclusive ao ligar o WAL, que o SQLite não faz esperar sozinho);
//! - a versão do schema fica no `user_version` ([`SCHEMA_VERSION`]): um
//!   binário antigo recusa um arquivo que ele não sabe ler.

use std::path::Path;
use std::time::{Duration, Instant};

use rusqlite::{Connection, ErrorCode, TransactionBehavior};

use crate::error::{Error, Result};

/// Quanto esperar por outro processo que está gravando.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
/// Intervalo entre as tentativas de ligar o WAL ([`enable_wal`]).
const WAL_RETRY_EVERY: Duration = Duration::from_millis(15);

/// A versão do schema: `major * 1000 + minor`.
///
/// - **minor** sobe a cada mudança que um binário mais antigo ainda lê (tabela
///   ou coluna nova, sempre com padrão): o antigo abre e segue.
/// - **major** sobe quando o antigo não pode mais mexer no arquivo (coluna
///   removida, significado trocado): ele recusa abrir, com erro claro.
pub const SCHEMA_VERSION: i32 = 1_000;

pub fn init_storage<P: AsRef<Path>>(db_path: P) -> Result<Connection> {
    let mut conn = Connection::open(db_path)?;
    conn.busy_timeout(BUSY_TIMEOUT)?;
    enable_wal(&conn)?;
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;

    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let found: i32 = tx.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if found / 1000 > SCHEMA_VERSION / 1000 {
        return Err(Error::Storage(format!(
            "app.db was written by a newer Octapus (schema {found}, this one reads up to {SCHEMA_VERSION}): update this app"
        )));
    }

    tx.execute_batch(
        r#"
        -- Grupos da sidebar (pastas de um nível). `servers.group_id` aponta
        -- para cá; excluir o grupo devolve os servidores para fora de grupo.
        CREATE TABLE IF NOT EXISTS server_groups (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT NOT NULL,
            created_at  INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
        );

        CREATE TABLE IF NOT EXISTS servers (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            name            TEXT NOT NULL,
            db_type         TEXT NOT NULL DEFAULT 'postgres',
            host            TEXT NOT NULL,
            port            INTEGER NOT NULL DEFAULT 5432,
            username        TEXT NOT NULL,
            password        TEXT NOT NULL,
            default_database TEXT,
            ssl_enabled     INTEGER NOT NULL DEFAULT 0,
            ssl_mode        TEXT,
            ssl_root_cert   TEXT,
            connection_uri  TEXT,
            created_at      INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
        );

        CREATE INDEX IF NOT EXISTS idx_servers_name ON servers(name);

        -- Sessão do workspace (abas abertas). Linha única: o id fixo em 1
        -- faz do salvar um upsert, sem histórico.
        CREATE TABLE IF NOT EXISTS workspace_session (
            id          INTEGER PRIMARY KEY CHECK (id = 1),
            snapshot    TEXT NOT NULL,
            updated_at  INTEGER NOT NULL
        );

        -- Preferências do app (models/settings.rs): uma linha por campo, com
        -- o valor em JSON. Campo novo não pede migração.
        CREATE TABLE IF NOT EXISTS app_settings (
            key     TEXT PRIMARY KEY,
            value   TEXT NOT NULL
        );

        -- Sessão do assistente de consulta: o token do participante, cifrado
        -- pelo vault, e a URL da API que o emitiu. Linha única; é a única
        -- coisa do assistente que vai para o disco (RN-01).
        CREATE TABLE IF NOT EXISTS assistant_session (
            id          INTEGER PRIMARY KEY CHECK (id = 1),
            api_url     TEXT NOT NULL,
            token       TEXT NOT NULL,
            created_at  INTEGER NOT NULL
        );
        "#,
    )?;

    run_migrations(&tx)?;
    // Nunca desce: um binário mais novo (minor maior) pode ter passado aqui
    if found < SCHEMA_VERSION {
        tx.execute_batch(&format!("PRAGMA user_version = {SCHEMA_VERSION}"))?;
    }
    tx.commit()?;

    Ok(conn)
}

/// Liga o WAL. Trocar o modo do journal pede a trava exclusiva do arquivo, e
/// o SQLite devolve "ocupado" **na hora**, sem passar pelo `busy_timeout`,
/// quando outro processo está no mesmo passo — é o que acontece com o app e o
/// terminal partindo juntos num `app.db` novo. Por isso a espera é feita aqui.
/// Com o arquivo já em WAL (toda partida depois da primeira) o pragma não
/// precisa de trava e passa direto.
fn enable_wal(conn: &Connection) -> Result<()> {
    let started = Instant::now();
    loop {
        match conn.query_row("PRAGMA journal_mode = WAL", [], |row| row.get::<_, String>(0)) {
            Ok(_) => return Ok(()),
            Err(error) if is_busy(&error) && started.elapsed() < BUSY_TIMEOUT => {
                std::thread::sleep(WAL_RETRY_EVERY);
            }
            Err(error) => return Err(error.into()),
        }
    }
}

fn is_busy(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(failure, _)
            if matches!(failure.code, ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked)
    )
}

/// As colunas que chegaram depois da primeira versão, pela ausência (um
/// `app.db` de antes do `user_version` não diz em que ponto parou).
fn run_migrations(conn: &Connection) -> Result<()> {
    let existing_columns: Vec<String> = {
        let mut stmt = conn.prepare("PRAGMA table_info(servers)")?;
        let cols = stmt.query_map([], |row| row.get::<_, String>(1))?;
        cols.filter_map(|c| c.ok()).collect()
    };

    let migrations: &[(&str, &str)] = &[
        ("db_type", "ALTER TABLE servers ADD COLUMN db_type TEXT NOT NULL DEFAULT 'postgres'"),
        ("ssl_enabled", "ALTER TABLE servers ADD COLUMN ssl_enabled INTEGER NOT NULL DEFAULT 0"),
        ("connection_uri", "ALTER TABLE servers ADD COLUMN connection_uri TEXT"),
        ("scope_databases", "ALTER TABLE servers ADD COLUMN scope_databases TEXT"),
        ("scope_schemas", "ALTER TABLE servers ADD COLUMN scope_schemas TEXT"),
        (
            "group_id",
            "ALTER TABLE servers ADD COLUMN group_id INTEGER \
             REFERENCES server_groups(id) ON DELETE SET NULL",
        ),
        ("ssl_mode", "ALTER TABLE servers ADD COLUMN ssl_mode TEXT"),
        ("ssl_root_cert", "ALTER TABLE servers ADD COLUMN ssl_root_cert TEXT"),
        (
            "assistant_sample_values",
            "ALTER TABLE servers ADD COLUMN assistant_sample_values INTEGER NOT NULL DEFAULT 0",
        ),
        ("display_timezone", "ALTER TABLE servers ADD COLUMN display_timezone TEXT"),
    ];

    for (col_name, alter_sql) in migrations {
        if !existing_columns.iter().any(|c| c == col_name) {
            conn.execute(alter_sql, [])?;
        }
    }

    // `ssl_enabled` (booleano) virou `ssl_mode`: ligado era TLS sem verificar
    // o certificado. O `ssl_enabled` continua gravado em par com o modo, para
    // uma versão anterior do app ainda ler o servidor.
    conn.execute(
        "UPDATE servers \
         SET ssl_mode = CASE WHEN ssl_enabled != 0 THEN 'require' ELSE 'disable' END \
         WHERE ssl_mode IS NULL",
        [],
    )?;

    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;

    fn temp_db(label: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("octapus-storage-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("app.db")
    }

    fn user_version(conn: &Connection) -> i32 {
        conn.query_row("PRAGMA user_version", [], |row| row.get(0)).unwrap()
    }

    #[test]
    fn stamps_the_schema_version_and_reopens() {
        let path = temp_db("version");
        assert_eq!(user_version(&init_storage(&path).unwrap()), SCHEMA_VERSION);
        assert_eq!(user_version(&init_storage(&path).unwrap()), SCHEMA_VERSION);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /// Um binário mais novo, compatível (minor maior), não é rebaixado; um
    /// incompatível (major maior) é recusado sem tocar no arquivo.
    #[test]
    fn newer_minor_is_kept_and_newer_major_is_refused() {
        let path = temp_db("skew");
        let conn = init_storage(&path).unwrap();
        conn.execute_batch(&format!("PRAGMA user_version = {}", SCHEMA_VERSION + 7)).unwrap();
        drop(conn);
        assert_eq!(user_version(&init_storage(&path).unwrap()), SCHEMA_VERSION + 7);

        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(&format!("PRAGMA user_version = {}", SCHEMA_VERSION + 1000)).unwrap();
        drop(conn);
        let error = init_storage(&path).unwrap_err();
        assert_eq!(error.code(), "STORAGE_ERROR");
        assert!(error.to_string().contains("newer Octapus"), "{error}");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /// Vários processos partindo juntos num arquivo novo: a criação do schema
    /// e as migrações fazem fila, nenhum falha.
    #[test]
    fn concurrent_first_runs_all_succeed() {
        let path = temp_db("race");
        std::thread::scope(|scope| {
            let handles: Vec<_> = (0..12).map(|_| scope.spawn(|| init_storage(&path).map(|_| ()))).collect();
            for handle in handles {
                handle.join().unwrap().unwrap();
            }
        });
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
