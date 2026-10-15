//! Grupos de servidores da sidebar: pastas de um nível, só para organizar.
//! O vínculo mora em `servers.group_id` (ver `servers::set_group`).

use parking_lot::Mutex;
use rusqlite::{params, Connection, Row};

use crate::error::{Error, Result};
use crate::models::ServerGroup;

const SELECT_COLUMNS: &str = "id, name, created_at";

/// Todos os grupos, em ordem alfabética (sem diferenciar maiúsculas).
pub fn get_all(storage: &Mutex<Connection>) -> Result<Vec<ServerGroup>> {
    let conn = storage.lock();
    let mut stmt = conn.prepare(&format!(
        "SELECT {SELECT_COLUMNS} FROM server_groups ORDER BY name COLLATE NOCASE ASC"
    ))?;
    let rows = stmt.query_map([], map_row)?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| Error::Storage(e.to_string()))
}

pub fn create(storage: &Mutex<Connection>, name: &str) -> Result<ServerGroup> {
    let conn = storage.lock();
    let name = valid_name(&conn, name, None)?;
    insert(&conn, &name)
}

pub fn rename(storage: &Mutex<Connection>, id: i64, name: &str) -> Result<ServerGroup> {
    let conn = storage.lock();
    let name = valid_name(&conn, name, Some(id))?;
    conn.query_row(
        &format!("UPDATE server_groups SET name = ?1 WHERE id = ?2 RETURNING {SELECT_COLUMNS}"),
        params![name, id],
        map_row,
    )
    .map_err(|e| not_found(e, id))
}

/// Exclui o grupo; os servidores dele ficam fora de grupo (não são excluídos).
pub fn delete(storage: &Mutex<Connection>, id: i64) -> Result<()> {
    let mut conn = storage.lock();
    let tx = conn.transaction()?;
    // O `ON DELETE SET NULL` já faria isso, mas a FK só vale com
    // `foreign_keys = ON` na conexão — aqui não depende disso.
    tx.execute(
        "UPDATE servers SET group_id = NULL WHERE group_id = ?",
        [id],
    )?;
    let affected = tx.execute("DELETE FROM server_groups WHERE id = ?", [id])?;
    if affected == 0 {
        return Err(Error::NotFound(format!("Group with id {} not found", id)));
    }
    tx.commit()?;
    Ok(())
}

/// Id do grupo com esse nome (sem diferenciar maiúsculas), criando-o se
/// preciso. A importação usa para recriar as pastas do app de origem.
pub fn find_or_create(conn: &Connection, name: &str) -> Result<i64> {
    let name = name.trim();
    if let Some(id) = find_by_name(conn, name, None)? {
        return Ok(id);
    }
    Ok(insert(conn, name)?.id)
}

fn insert(conn: &Connection, name: &str) -> Result<ServerGroup> {
    conn.query_row(
        &format!("INSERT INTO server_groups (name) VALUES (?1) RETURNING {SELECT_COLUMNS}"),
        [name],
        map_row,
    )
    .map_err(|e| Error::Storage(e.to_string()))
}

/// Nome aparado, não vazio e sem repetir outro grupo.
fn valid_name(conn: &Connection, name: &str, except: Option<i64>) -> Result<String> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Error::InvalidState(
            "O nome do grupo não pode ficar vazio".into(),
        ));
    }
    if find_by_name(conn, name, except)?.is_some() {
        return Err(Error::AlreadyExists(format!(
            "Já existe um grupo chamado \"{name}\""
        )));
    }
    Ok(name.to_string())
}

/// Busca sem diferenciar maiúsculas, em Rust: o `NOCASE` do SQLite só dobra
/// ASCII ("Produção" ≠ "PRODUÇÃO" para ele). São poucos grupos.
fn find_by_name(conn: &Connection, name: &str, except: Option<i64>) -> Result<Option<i64>> {
    let needle = name.to_lowercase();
    let mut stmt = conn.prepare("SELECT id, name FROM server_groups")?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let id: i64 = row.get(0)?;
        let existing: String = row.get(1)?;
        if Some(id) != except && existing.to_lowercase() == needle {
            return Ok(Some(id));
        }
    }
    Ok(None)
}

fn map_row(row: &Row<'_>) -> rusqlite::Result<ServerGroup> {
    Ok(ServerGroup {
        id: row.get(0)?,
        name: row.get(1)?,
        created_at: row.get(2)?,
    })
}

fn not_found(e: rusqlite::Error, id: i64) -> Error {
    match e {
        rusqlite::Error::QueryReturnedNoRows => {
            Error::NotFound(format!("Group with id {} not found", id))
        }
        _ => Error::Storage(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{DatabaseType, ServerInput};
    use crate::storage::database::init_storage;
    use crate::storage::repositories::servers;
    use crate::storage::vault;

    fn storage() -> Mutex<Connection> {
        vault::init_for_tests();
        Mutex::new(init_storage(":memory:").unwrap())
    }

    fn server_in(storage: &Mutex<Connection>, group_id: Option<i64>) -> i64 {
        servers::create(
            storage,
            ServerInput {
                name: "srv".into(),
                db_type: DatabaseType::Postgres,
                host: "localhost".into(),
                port: 5432,
                username: "u".into(),
                password: "p".into(),
                default_database: None,
                ssl_enabled: None,
                connection_uri: None,
                scope_databases: None,
                scope_schemas: None,
                group_id,
            },
        )
        .unwrap()
        .id
        .unwrap()
    }

    #[test]
    fn crud_and_unique_names() {
        let storage = storage();
        let a = create(&storage, "  Produção ").unwrap();
        assert_eq!(a.name, "Produção");
        create(&storage, "local").unwrap();

        assert!(matches!(
            create(&storage, "PRODUÇÃO"),
            Err(Error::AlreadyExists(_))
        ));
        assert!(matches!(
            create(&storage, "Local"),
            Err(Error::AlreadyExists(_))
        ));
        assert!(matches!(
            create(&storage, "   "),
            Err(Error::InvalidState(_))
        ));

        // Renomear para o próprio nome (outra caixa) não conflita consigo
        assert_eq!(rename(&storage, a.id, "produção").unwrap().name, "produção");
        assert!(matches!(
            rename(&storage, a.id, "LOCAL"),
            Err(Error::AlreadyExists(_))
        ));
        assert!(matches!(
            rename(&storage, 999, "x"),
            Err(Error::NotFound(_))
        ));

        let names: Vec<_> = get_all(&storage)
            .unwrap()
            .into_iter()
            .map(|g| g.name)
            .collect();
        assert_eq!(names, ["local", "produção"]);
    }

    #[test]
    fn move_and_delete_keep_servers() {
        let storage = storage();
        let g1 = create(&storage, "g1").unwrap().id;
        let g2 = create(&storage, "g2").unwrap().id;
        let id = server_in(&storage, Some(g1));
        assert_eq!(
            servers::get_by_id_meta(&storage, id).unwrap().group_id,
            Some(g1)
        );

        // Já num grupo, muda de grupo e sai de grupo
        servers::set_group(&storage, id, Some(g2)).unwrap();
        assert_eq!(
            servers::get_by_id_meta(&storage, id).unwrap().group_id,
            Some(g2)
        );
        servers::set_group(&storage, id, None).unwrap();
        assert_eq!(
            servers::get_by_id_meta(&storage, id).unwrap().group_id,
            None
        );

        assert!(servers::set_group(&storage, id, Some(999)).is_err());
        assert!(matches!(
            servers::set_group(&storage, 999, Some(g1)),
            Err(Error::NotFound(_))
        ));

        // Excluir o grupo devolve o servidor para fora de grupo, sem excluí-lo
        servers::set_group(&storage, id, Some(g1)).unwrap();
        delete(&storage, g1).unwrap();
        assert_eq!(
            servers::get_by_id_meta(&storage, id).unwrap().group_id,
            None
        );
        assert!(matches!(delete(&storage, g1), Err(Error::NotFound(_))));
    }

    #[test]
    fn find_or_create_reuses_by_name() {
        let storage = storage();
        let conn = storage.lock();
        let a = find_or_create(&conn, "Staging").unwrap();
        assert_eq!(find_or_create(&conn, " staging ").unwrap(), a);
        assert_ne!(find_or_create(&conn, "Prod").unwrap(), a);
    }
}
