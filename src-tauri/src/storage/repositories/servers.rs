use std::time::{SystemTime, UNIX_EPOCH};

use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::error::{Error, Result};
use crate::models::{DatabaseType, Server, ServerInput};
use crate::storage::{health, secrets, vault};

const SELECT_COLUMNS: &str = "id, name, db_type, host, port, username, password, \
                              default_database, ssl_enabled, connection_uri, created_at";

/// Máscara que substitui a senha embutida numa URI antes de ela ir para o front.
/// Também é o valor devolvido quando o cofre não conseguiu decifrar (chave
/// ausente/corrompida): melhor um campo opaco do que vazar ou sumir em silêncio.
const URI_MASK: &str = "••••";

/// Get all servers (metadata only). The UI never displays passwords, so the
/// stored ciphertext is never decrypted nor returned here; the connection URI
/// comes back **redigida** (sem a senha embutida).
pub fn get_all(storage: &Mutex<Connection>) -> Result<Vec<Server>> {
    let conn = storage.lock();

    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLUMNS} FROM servers ORDER BY name ASC"
    ))?;

    let rows = stmt.query_map([], map_row)?;
    let mut servers = rows
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::Storage(e.to_string()))?;

    // Never expose the stored ciphertext.
    for server in &mut servers {
        server.password.clear();
        redact_uri_for_display(&conn, server);
    }

    Ok(servers)
}

/// Get a server by ID with its **decrypted** password and connection URI
/// (needed to open a pool). Prefer [`get_by_id_meta`] when only metadata is
/// required — este é o único caminho em que a URI existe em texto puro.
pub fn get_by_id(storage: &Mutex<Connection>, id: i64) -> Result<Server> {
    // Trancado ≠ quebrado: aqui o front abre o diálogo de senha mestre em vez
    // de dizer que as senhas se perderam.
    if vault::is_locked() {
        return Err(Error::VaultLocked);
    }

    let conn = storage.lock();
    let mut server = select_one(&conn, id)?;
    decrypt_password(&conn, &mut server)?;
    decrypt_uri(&conn, &mut server)?;
    Ok(server)
}

/// Get a server by ID without decrypting the password — use it for metadata-only
/// needs (db type, default database). The `password` field comes back empty and
/// the URI vem redigida.
pub fn get_by_id_meta(storage: &Mutex<Connection>, id: i64) -> Result<Server> {
    let conn = storage.lock();
    let mut server = select_one(&conn, id)?;
    server.password.clear();
    redact_uri_for_display(&conn, &mut server);
    Ok(server)
}

fn select_one(conn: &Connection, id: i64) -> Result<Server> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLUMNS} FROM servers WHERE id = ?"
    ))?;

    stmt.query_row([id], map_row).map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            Error::NotFound(format!("Server with id {} not found", id))
        }
        _ => Error::Storage(e.to_string()),
    })
}

/// Create a new server. A senha **e** a URI de conexão são criptografadas no
/// cofre antes de serem gravadas — as colunas do SQLite só guardam ciphertext.
pub fn create(storage: &Mutex<Connection>, mut input: ServerInput) -> Result<Server> {
    let conn = storage.lock();

    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| Error::Storage(e.to_string()))?
        .as_secs() as i64;

    let db_type_str = db_type_to_string(&input.db_type);
    let ssl_enabled = input.ssl_enabled.unwrap_or(false) as i32;
    let encrypted = vault::encrypt(&input.password)?;
    let encrypted_uri = encrypt_uri(input.connection_uri.as_deref())?;

    let mut stmt = conn.prepare(&format!(
        "INSERT INTO servers (name, db_type, host, port, username, password, \
                              default_database, ssl_enabled, connection_uri, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10) \
         RETURNING {SELECT_COLUMNS}"
    ))?;

    let mut server = stmt
        .query_row(
            params![
                input.name,
                db_type_str,
                input.host,
                input.port,
                input.username,
                encrypted,
                input.default_database,
                ssl_enabled,
                encrypted_uri,
                created_at,
            ],
            map_row,
        )
        .map_err(|e| Error::Storage(e.to_string()))?;

    // Keep the plaintext in the returned value for immediate use.
    // `mem::take` porque `ServerInput` tem `Drop` (zeroize) e não permite
    // mover o campo para fora — o buffer é o mesmo, só troca de dono.
    server.password = std::mem::take(&mut input.password);
    // A URI, essa sim é serializada para o front — devolve redigida.
    redact_uri_for_display(&conn, &mut server);

    Ok(server)
}

/// Update an existing server. A senha e a URI são criptografadas no cofre antes
/// de serem gravadas.
pub fn update(storage: &Mutex<Connection>, id: i64, mut input: ServerInput) -> Result<Server> {
    let conn = storage.lock();

    let db_type_str = db_type_to_string(&input.db_type);
    let ssl_enabled = input.ssl_enabled.unwrap_or(false) as i32;
    let encrypted = vault::encrypt(&input.password)?;
    let stored_uri = uri_to_store(&conn, id, input.connection_uri.as_deref())?;

    let mut stmt = conn.prepare(&format!(
        "UPDATE servers \
         SET name = ?1, db_type = ?2, host = ?3, port = ?4, username = ?5, \
             password = ?6, default_database = ?7, ssl_enabled = ?8, connection_uri = ?9 \
         WHERE id = ?10 \
         RETURNING {SELECT_COLUMNS}"
    ))?;

    let mut server = stmt
        .query_row(
            params![
                input.name,
                db_type_str,
                input.host,
                input.port,
                input.username,
                encrypted,
                input.default_database,
                ssl_enabled,
                stored_uri,
                id,
            ],
            map_row,
        )
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                Error::NotFound(format!("Server with id {} not found", id))
            }
            _ => Error::Storage(e.to_string()),
        })?;

    server.password = std::mem::take(&mut input.password);
    redact_uri_for_display(&conn, &mut server);

    Ok(server)
}

/// Delete a server. The encrypted secret lives in the row, so it goes away with
/// it; we also drop any legacy keychain entry best-effort.
pub fn delete(storage: &Mutex<Connection>, id: i64) -> Result<()> {
    let conn = storage.lock();

    let affected = conn.execute("DELETE FROM servers WHERE id = ?", [id])?;

    if affected == 0 {
        return Err(Error::NotFound(format!("Server with id {} not found", id)));
    }

    secrets::delete_password(id);

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn map_row(row: &Row<'_>) -> rusqlite::Result<Server> {
    Ok(Server {
        id: Some(row.get(0)?),
        name: row.get(1)?,
        db_type: parse_db_type(row.get::<_, String>(2)?),
        host: row.get(3)?,
        port: row.get(4)?,
        username: row.get(5)?,
        password: row.get(6)?,
        default_database: row.get(7)?,
        ssl_enabled: row.get::<_, i32>(8)? != 0,
        connection_uri: row.get(9)?,
        created_at: row.get(10)?,
    })
}

/// Replace `server.password` (the stored value) with the plaintext password.
///
/// - vault envelope → decrypt;
/// - empty column → legacy keychain install: read it once (last OS prompt),
///   re-encrypt into the vault and drop the keychain entry;
/// - plaintext column (pre-keychain install) → encrypt into the vault in place.
fn decrypt_password(conn: &Connection, server: &mut Server) -> Result<()> {
    let Some(id) = server.id else { return Ok(()) };
    let stored = std::mem::take(&mut server.password);

    if vault::is_envelope(&stored) {
        // Envelope que não abre = chave errada. Falha alto e com mensagem clara,
        // em vez de deixar a senha vazia e o banco recusar a conexão depois.
        server.password = vault::decrypt(&stored).ok_or(Error::VaultUnavailable)?;
        return Ok(());
    }

    // Legacy value: keychain (empty column) or plaintext column.
    let from_plaintext_column = !stored.is_empty();
    let plaintext = if from_plaintext_column {
        Some(stored)
    } else {
        secrets::get_password(id)
    };

    let Some(plain) = plaintext else { return Ok(()) };

    // Migrate into the vault so future reads never touch the keychain again.
    if let Ok(envelope) = vault::encrypt(&plain) {
        let migrated = conn.execute(
            "UPDATE servers SET password = ?1 WHERE id = ?2",
            params![envelope, id],
        );
        secrets::delete_password(id);

        // A senha vinda do keychain nunca esteve no arquivo; a que estava na
        // coluna deixou resíduo em páginas livres, e só o VACUUM tira.
        if migrated.is_ok() && from_plaintext_column {
            health::mark_pending_vacuum(conn);
        }
    }

    server.password = plain;
    Ok(())
}

/// Cifra a URI para gravação. `None`/vazio continuam `None` na coluna.
fn encrypt_uri(uri: Option<&str>) -> Result<Option<String>> {
    match uri.map(str::trim).filter(|u| !u.is_empty()) {
        Some(uri) => Ok(Some(vault::encrypt(uri)?)),
        None => Ok(None),
    }
}

/// Resolve o valor a gravar na coluna `connection_uri` num update.
///
/// O front recebe a URI **redigida**; se o usuário não tocar no campo, ela volta
/// exatamente como saiu. Nesse caso preservamos o ciphertext já guardado em vez
/// de gravar a máscara por cima da credencial real.
fn uri_to_store(conn: &Connection, id: i64, incoming: Option<&str>) -> Result<Option<String>> {
    let Some(incoming) = incoming.map(str::trim).filter(|u| !u.is_empty()) else {
        return Ok(None);
    };

    let stored = current_uri_column(conn, id)?;

    if let Some(stored) = stored {
        let unchanged = match plain_uri(&stored) {
            Some(plain) => redact_uri(&plain) == incoming,
            // Cofre indisponível: só a máscara pura pode ter sido exibida.
            None => incoming == URI_MASK,
        };

        if unchanged {
            return Ok(Some(stored));
        }
    }

    Ok(Some(vault::encrypt(incoming)?))
}

/// Lê a coluna `connection_uri` crua (como está gravada) de um servidor.
fn current_uri_column(conn: &Connection, id: i64) -> Result<Option<String>> {
    let stored = conn
        .query_row(
            "SELECT connection_uri FROM servers WHERE id = ?",
            [id],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
        .map_err(|e| Error::Storage(e.to_string()))?
        .flatten();

    Ok(stored.filter(|s| !s.is_empty()))
}

/// Texto puro de uma coluna `connection_uri`, seja ela envelope ou legado.
fn plain_uri(stored: &str) -> Option<String> {
    if vault::is_envelope(stored) {
        vault::decrypt(stored)
    } else {
        Some(stored.to_string())
    }
}

/// Substitui a coluna armazenada pela URI em texto puro, migrando valores
/// legados (URI em texto puro na coluna) para o cofre no caminho.
fn decrypt_uri(conn: &Connection, server: &mut Server) -> Result<()> {
    server.connection_uri = take_and_migrate_uri(conn, server)?;
    Ok(())
}

/// Valor de exibição da URI: nunca contém a senha embutida. Usado em todo
/// caminho que serializa o `Server` para o front.
fn redact_uri_for_display(conn: &Connection, server: &mut Server) {
    let had_uri = server.connection_uri.as_deref().is_some_and(|u| !u.is_empty());

    match take_and_migrate_uri(conn, server) {
        Ok(Some(plain)) => server.connection_uri = Some(redact_uri(&plain)),
        // Havia URI mas o cofre não decifrou: mostra opaco em vez de sumir.
        // A lista de servidores continua abrindo mesmo com a chave quebrada.
        Err(_) if had_uri => server.connection_uri = Some(URI_MASK.to_string()),
        _ => {}
    }
}

/// Tira a URI do `server`, migra a coluna para o cofre se ela ainda estiver em
/// texto puro e devolve o plaintext (ou `None` se não houver / não decifrar).
fn take_and_migrate_uri(conn: &Connection, server: &mut Server) -> Result<Option<String>> {
    let Some(id) = server.id else { return Ok(None) };
    let Some(stored) = server.connection_uri.take().filter(|s| !s.is_empty()) else {
        return Ok(None);
    };

    if vault::is_envelope(&stored) {
        return vault::decrypt(&stored).map(Some).ok_or(Error::VaultUnavailable);
    }

    // Legado: URI em texto puro na coluna → cifra em lugar, uma única vez.
    if let Ok(envelope) = vault::encrypt(&stored) {
        let migrated = conn.execute(
            "UPDATE servers SET connection_uri = ?1 WHERE id = ?2",
            params![envelope, id],
        );

        // Sobrou a URI antiga em páginas livres do arquivo; VACUUM no próximo boot.
        if migrated.is_ok() {
            health::mark_pending_vacuum(conn);
        }
    }

    Ok(Some(stored))
}

/// Devolve a URI sem a senha embutida.
///
/// `mongodb+srv://admin:s3cret@cluster.net/db` → `mongodb+srv://admin:••••@cluster.net/db`
///
/// URIs sem credencial voltam intactas; o que não for parseável vira máscara
/// pura, já que não dá para saber onde a senha termina.
fn redact_uri(uri: &str) -> String {
    let Some(scheme_end) = uri.find("://") else {
        return URI_MASK.to_string();
    };
    let authority_start = scheme_end + 3;

    // A autoridade termina no primeiro `/`, `?` ou `#` depois do esquema.
    let authority_end = uri[authority_start..]
        .find(['/', '?', '#'])
        .map_or(uri.len(), |i| authority_start + i);

    let authority = &uri[authority_start..authority_end];

    // O separador do userinfo é o **último** `@`: o host não pode conter `@`,
    // mas a senha pode (percent-encoding não é obrigatório na prática).
    let Some(at) = authority.rfind('@') else {
        return uri.to_string();
    };

    let userinfo = &authority[..at];
    let Some(colon) = userinfo.find(':') else {
        return uri.to_string();
    };

    format!(
        "{}{}:{}{}",
        &uri[..authority_start],
        &userinfo[..colon],
        URI_MASK,
        &uri[authority_start + at..],
    )
}

fn parse_db_type(s: String) -> DatabaseType {
    match s.to_lowercase().as_str() {
        "mysql" => DatabaseType::Mysql,
        "sqlite" => DatabaseType::Sqlite,
        "mongodb" => DatabaseType::Mongodb,
        "redis" => DatabaseType::Redis,
        _ => DatabaseType::Postgres,
    }
}

fn db_type_to_string(db_type: &DatabaseType) -> &'static str {
    match db_type {
        DatabaseType::Postgres => "postgres",
        DatabaseType::Mysql => "mysql",
        DatabaseType::Sqlite => "sqlite",
        DatabaseType::Mongodb => "mongodb",
        DatabaseType::Redis => "redis",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::database::init_storage;

    const URI: &str = "mongodb+srv://admin:s3cret@cluster.mongodb.net/db?retryWrites=true";
    const URI_REDACTED: &str = "mongodb+srv://admin:••••@cluster.mongodb.net/db?retryWrites=true";

    fn sample_input() -> ServerInput {
        ServerInput {
            name: "vault-test".into(),
            db_type: DatabaseType::Postgres,
            host: "localhost".into(),
            port: 5432,
            username: "user".into(),
            password: "s3cret".into(),
            default_database: None,
            ssl_enabled: None,
            connection_uri: None,
        }
    }

    fn input_with_uri() -> ServerInput {
        let mut input = sample_input();
        input.connection_uri = Some(URI.into());
        input
    }

    /// `..base` não compila com `Drop`; monta o input e ajusta a URI.
    fn input_with(uri: Option<String>) -> ServerInput {
        let mut input = sample_input();
        input.connection_uri = uri;
        input
    }

    fn storage() -> Mutex<Connection> {
        vault::init_for_tests();
        Mutex::new(init_storage(":memory:").unwrap())
    }

    fn column(storage: &Mutex<Connection>, name: &str, id: i64) -> Option<String> {
        storage
            .lock()
            .query_row(
                &format!("SELECT {name} FROM servers WHERE id = ?"),
                [id],
                |r| r.get::<_, Option<String>>(0),
            )
            .unwrap()
    }

    #[test]
    fn password_is_encrypted_at_rest() {
        let storage = storage();

        let created = create(&storage, sample_input()).unwrap();
        let id = created.id.unwrap();

        // The returned value keeps the plaintext for immediate use.
        assert_eq!(created.password, "s3cret");

        // The SQLite column stores ciphertext, never the plaintext.
        let stored = column(&storage, "password", id).unwrap();
        assert_ne!(stored, "s3cret", "password leaked to SQLite column");
        assert!(vault::is_envelope(&stored), "expected a vault envelope");

        // Round-trip: get_by_id decrypts back to the plaintext.
        assert_eq!(get_by_id(&storage, id).unwrap().password, "s3cret");

        // Metadata paths never expose the secret.
        assert_eq!(get_by_id_meta(&storage, id).unwrap().password, "");
        assert_eq!(get_all(&storage).unwrap()[0].password, "");
    }

    #[test]
    fn connection_uri_is_encrypted_at_rest() {
        let storage = storage();

        let created = create(&storage, input_with_uri()).unwrap();
        let id = created.id.unwrap();

        // A coluna guarda só ciphertext — nada de senha em texto puro no SQLite.
        let stored = column(&storage, "connection_uri", id).unwrap();
        assert!(vault::is_envelope(&stored), "expected a vault envelope");
        assert!(!stored.contains("s3cret"), "URI password leaked to SQLite");

        // O caminho de conexão recebe a URI inteira.
        assert_eq!(
            get_by_id(&storage, id).unwrap().connection_uri.as_deref(),
            Some(URI)
        );

        // Todo caminho que serializa para o front devolve redigido.
        assert_eq!(created.connection_uri.as_deref(), Some(URI_REDACTED));
        assert_eq!(
            get_all(&storage).unwrap()[0].connection_uri.as_deref(),
            Some(URI_REDACTED)
        );
        assert_eq!(
            get_by_id_meta(&storage, id).unwrap().connection_uri.as_deref(),
            Some(URI_REDACTED)
        );
    }

    #[test]
    fn legacy_plaintext_uri_is_migrated_on_read() {
        let storage = storage();
        let id = create(&storage, sample_input()).unwrap().id.unwrap();

        // Simula uma instalação anterior: URI em texto puro na coluna.
        storage
            .lock()
            .execute(
                "UPDATE servers SET connection_uri = ?1 WHERE id = ?2",
                params![URI, id],
            )
            .unwrap();

        // A primeira leitura migra a linha para o cofre, sem perder o valor.
        assert_eq!(
            get_all(&storage).unwrap()[0].connection_uri.as_deref(),
            Some(URI_REDACTED)
        );

        let stored = column(&storage, "connection_uri", id).unwrap();
        assert!(vault::is_envelope(&stored), "legacy URI was not migrated");
        assert_eq!(
            get_by_id(&storage, id).unwrap().connection_uri.as_deref(),
            Some(URI)
        );
    }

    #[test]
    fn unchanged_redacted_uri_keeps_stored_credentials() {
        let storage = storage();
        let id = create(&storage, input_with_uri()).unwrap().id.unwrap();

        // O front devolve exatamente o que recebeu (campo intocado).
        let from_front = get_all(&storage).unwrap()[0].connection_uri.clone();
        assert_eq!(from_front.as_deref(), Some(URI_REDACTED));

        update(&storage, id, input_with(from_front)).unwrap();

        // A credencial real sobreviveu — a máscara não foi gravada por cima.
        assert_eq!(
            get_by_id(&storage, id).unwrap().connection_uri.as_deref(),
            Some(URI)
        );
    }

    #[test]
    fn edited_uri_replaces_stored_credentials() {
        let storage = storage();
        let id = create(&storage, input_with_uri()).unwrap().id.unwrap();

        let novo = "mongodb+srv://admin:outra@cluster.mongodb.net/db";
        update(&storage, id, input_with(Some(novo.into()))).unwrap();

        assert_eq!(
            get_by_id(&storage, id).unwrap().connection_uri.as_deref(),
            Some(novo)
        );

        let stored = column(&storage, "connection_uri", id).unwrap();
        assert!(!stored.contains("outra"), "new URI password leaked");
    }

    #[test]
    fn clearing_the_uri_wipes_the_column() {
        let storage = storage();
        let id = create(&storage, input_with_uri()).unwrap().id.unwrap();

        update(&storage, id, sample_input()).unwrap();

        assert_eq!(column(&storage, "connection_uri", id), None);
    }

    #[test]
    fn redact_uri_covers_the_usual_shapes() {
        // Caso comum: usuário e senha embutidos.
        assert_eq!(redact_uri(URI), URI_REDACTED);

        // Redis sem usuário.
        assert_eq!(
            redact_uri("rediss://:senha@redis.cloud:6380"),
            "rediss://:••••@redis.cloud:6380"
        );

        // Sem credencial nenhuma → volta intacta.
        assert_eq!(
            redact_uri("postgres://localhost:5432/app"),
            "postgres://localhost:5432/app"
        );

        // Só usuário, sem senha → volta intacta.
        assert_eq!(
            redact_uri("mongodb://admin@host:27017"),
            "mongodb://admin@host:27017"
        );

        // `@` dentro da senha: o separador é o último, não o primeiro.
        assert_eq!(
            redact_uri("mongodb://user:p@ss@host:27017/db"),
            "mongodb://user:••••@host:27017/db"
        );

        // `@` no path não pode ser confundido com userinfo.
        assert_eq!(
            redact_uri("postgres://user:pw@host/db@weird"),
            "postgres://user:••••@host/db@weird"
        );

        // Ilegível → máscara pura, para não vazar por engano.
        assert_eq!(redact_uri("isso-nao-e-uma-uri"), "••••");
    }

    /// Envelope bem formado, mas cifrado com outra chave (simula `vault.key`
    /// trocado ou perdido).
    const FOREIGN: &str = "v1:deadbeefdeadbeefdeadbeefdeadbeefdeadbeef";

    #[test]
    fn unreadable_password_fails_loudly_instead_of_silently_empty() {
        let storage = storage();
        let id = create(&storage, sample_input()).unwrap().id.unwrap();

        storage
            .lock()
            .execute(
                "UPDATE servers SET password = ?1 WHERE id = ?2",
                params![FOREIGN, id],
            )
            .unwrap();

        // Antes isso virava senha vazia e o banco respondia "authentication
        // failed"; agora o erro diz o que realmente aconteceu.
        let err = get_by_id(&storage, id).unwrap_err();
        assert_eq!(err.code(), "VAULT_UNAVAILABLE");

        // Mas a lista de servidores continua abrindo.
        assert!(get_all(&storage).is_ok());
    }

    #[test]
    fn unreadable_uri_stays_opaque_in_the_list() {
        let storage = storage();
        let id = create(&storage, input_with_uri()).unwrap().id.unwrap();

        storage
            .lock()
            .execute(
                "UPDATE servers SET connection_uri = ?1 WHERE id = ?2",
                params![FOREIGN, id],
            )
            .unwrap();

        assert_eq!(
            get_all(&storage).unwrap()[0].connection_uri.as_deref(),
            Some(URI_MASK)
        );
        assert_eq!(
            get_by_id(&storage, id).unwrap_err().code(),
            "VAULT_UNAVAILABLE"
        );
    }

    #[test]
    fn debug_never_prints_secrets() {
        let storage = storage();
        let created = create(&storage, input_with_uri()).unwrap();

        // O `created` ainda carrega a senha em claro para uso imediato.
        assert_eq!(created.password, "s3cret");

        let dump = format!("{created:?}");
        assert!(!dump.contains("s3cret"), "senha vazou no Debug: {dump}");
        assert!(!dump.contains("cluster.mongodb.net"), "URI vazou: {dump}");
        assert!(dump.contains("<redigido>"), "esperava marcador: {dump}");

        // O input também não pode falar demais.
        let dump = format!("{:?}", input_with_uri());
        assert!(!dump.contains("s3cret"), "senha vazou no Debug do input: {dump}");
    }
}
