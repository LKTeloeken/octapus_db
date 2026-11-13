//! Beekeeper Studio: tabela `saved_connection` do `app.db` (SQLite). As senhas
//! vêm cifradas com a chave da instalação, guardada no `.key` (ver `crypto`).
//! Só o workspace local (`workspaceId = -1`); os da nuvem têm dono lá.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};

use super::mapping::{canonical_driver, RawConnection, Secret};
use super::sqlite_util::{select_existing, table_exists};
use super::{crypto, paths};
use crate::error::{Error, Result};

pub fn find_file() -> Option<PathBuf> {
    paths::beekeeper_dir()
        .map(|dir| dir.join("app.db"))
        .filter(|path| path.is_file())
}

pub fn read(file: &Path) -> Result<Vec<RawConnection>> {
    let install_key = file
        .parent()
        .and_then(|dir| std::fs::read_to_string(dir.join(".key")).ok())
        .and_then(|key_file| crypto::beekeeper_install_key(&key_file));

    let conn = Connection::open_with_flags(file, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .map_err(|e| Error::InvalidState(format!("Não deu para abrir {}: {e}", file.display())))?;
    if !table_exists(&conn, "saved_connection")? {
        return Err(Error::InvalidState(format!(
            "{} não é um banco do Beekeeper Studio",
            file.display()
        )));
    }

    let folders: HashMap<i64, String> = if table_exists(&conn, "connection_folder")? {
        let mut stmt = conn.prepare("SELECT id, name FROM connection_folder")?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.filter_map(|row| row.ok()).collect()
    } else {
        HashMap::new()
    };

    let columns = [
        "name",
        "connectionType",
        "host",
        "port",
        "username",
        "defaultDatabase",
        "password",
        "ssl",
        "sslRejectUnauthorized",
        "sslCertFile",
        "sshEnabled",
        "sshHost",
        "connectionFolderId",
        "workspaceId",
    ];
    let sql = format!(
        "SELECT {} FROM saved_connection ORDER BY id",
        select_existing(&conn, "saved_connection", &columns)?
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], |row| {
        let text = |i: usize| -> rusqlite::Result<Option<String>> {
            Ok(row.get::<_, Option<String>>(i)?.filter(|s| !s.is_empty()))
        };
        let flag = |i: usize| -> rusqlite::Result<bool> {
            Ok(row.get::<_, Option<i64>>(i)?.unwrap_or(0) != 0)
        };
        // Sem coluna de workspace (versão antiga) tudo é local
        let workspace: Option<i64> = row.get(13)?;
        if workspace.is_some_and(|id| id != -1) {
            return Ok(None);
        }

        let mut raw = RawConnection {
            host: text(2)?.unwrap_or_default(),
            port: row
                .get::<_, Option<i64>>(3)?
                .and_then(|p| u16::try_from(p).ok()),
            username: text(4)?.unwrap_or_default(),
            database: text(5)?,
            group: row
                .get::<_, Option<i64>>(12)?
                .and_then(|id| folders.get(&id).cloned()),
            ..RawConnection::new(
                text(0)?.unwrap_or_else(|| "Sem nome".into()),
                canonical_driver(&text(1)?.unwrap_or_default()),
            )
        };
        if flag(7)? {
            // Rejeitar certificado inválido = verificar
            raw.ssl_mode = Some(if flag(8)? { "verify-full" } else { "require" }.into());
            raw.client_certificate = text(9)?.is_some();
        }
        if flag(10)? {
            raw.ssh_host = Some(text(11)?.unwrap_or_default());
        }
        if let (Some(payload), Some(key)) = (text(6)?, install_key.as_deref()) {
            if let Some(password) = crypto::beekeeper_decrypt_string(&payload, key) {
                raw.secret = Secret::Plain(password);
            }
        }
        Ok(Some(raw))
    })?;
    Ok(rows.filter_map(|row| row.ok().flatten()).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::connection_import::crypto::test_support::{
        beekeeper_key_file, encrypt_simple,
    };

    #[test]
    fn reads_local_workspace_with_passwords_and_folders() {
        let dir = std::env::temp_dir().join(format!("octapus-beekeeper-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("app.db");
        let _ = std::fs::remove_file(&file);
        std::fs::write(dir.join(".key"), beekeeper_key_file("k-123")).unwrap();
        let password = encrypt_simple(br#""s3cret""#, "k-123");
        {
            let conn = Connection::open(&file).unwrap();
            conn.execute_batch(
                "CREATE TABLE connection_folder (id INTEGER PRIMARY KEY, name TEXT);
                 INSERT INTO connection_folder VALUES (5, 'Clientes');
                 CREATE TABLE saved_connection (id INTEGER PRIMARY KEY, name TEXT,
                     connectionType TEXT, host TEXT, port INTEGER, username TEXT,
                     defaultDatabase TEXT, password TEXT, ssl INTEGER,
                     sslRejectUnauthorized INTEGER, sshEnabled INTEGER, sshHost TEXT,
                     connectionFolderId INTEGER, workspaceId INTEGER);",
            )
            .unwrap();
            conn.execute(
                "INSERT INTO saved_connection VALUES
                    (1, 'ERP', 'postgresql', 'db.erp', 5432, 'app', 'erp', ?1, 1, 0, 1, 'bastion', 5, -1),
                    (2, 'Nuvem', 'mysql', 'x', 3306, 'u', NULL, NULL, 0, 0, 0, NULL, NULL, 42)",
                [password],
            )
            .unwrap();
        }

        let raws = read(&file).unwrap();
        assert_eq!(raws.len(), 1, "o workspace da nuvem fica de fora");
        let erp = &raws[0];
        assert_eq!(erp.driver, "PostgreSQL");
        assert_eq!(erp.group.as_deref(), Some("Clientes"));
        assert_eq!(erp.ssl_mode.as_deref(), Some("require"));
        assert_eq!(erp.ssh_host.as_deref(), Some("bastion"));
        assert_eq!(erp.secret, Secret::Plain("s3cret".into()));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
