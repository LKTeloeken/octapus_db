//! Catálogo do SQLite (refactor do catálogo, Fase 5).
//!
//! Sem nível de schema: as tabelas e views do arquivo vão para o
//! [`FLAT_SCHEMA`]. O fingerprint é o `schema_version`, que o próprio SQLite
//! incrementa a cada mudança de estrutura. A conexão é só do catálogo (o
//! rusqlite é bloqueante e o editor não pode esperar por ela).

use std::sync::Arc;
use std::time::Instant;

use async_trait::async_trait;
use parking_lot::{Mutex, RwLock};
use rusqlite::{Connection, InterruptHandle, OptionalExtension};

use crate::catalog::{
    sync_flat, Catalog, CatalogSource, ColumnDetail, ColumnMatch, ForeignKey, ForeignKeyObject, IndexObject,
    ObjectKind, ObjectRef, RelKind, RelationDetail, SourceCanceller, SyncEvent, SyncReport, TableRecord,
    TriggerObject, FLAT_SCHEMA,
};
use crate::error::{Error, Result};
use crate::models::Server;

use super::util::{db_err, quote_ident};

pub struct SqliteSource {
    conn: Arc<Mutex<Connection>>,
    interrupt: Arc<InterruptHandle>,
    /// `main`, `temp` ou um banco anexado (o `ATTACH` é da conexão do editor,
    /// então aqui só o `main` costuma ter conteúdo)
    database: String,
    version: String,
}

impl SqliteSource {
    pub async fn connect(server: &Server, database: &str) -> Result<Self> {
        let path = super::resolve_path(server)?;
        let conn = tokio::task::spawn_blocking(move || super::open(&path))
            .await
            .map_err(|e| Error::InvalidState(format!("SQLite task failed: {e}")))??;

        let version: String = conn
            .query_row("SELECT sqlite_version()", [], |row| row.get(0))
            .map_err(db_err)?;

        Ok(Self {
            interrupt: Arc::new(conn.get_interrupt_handle()),
            conn: Arc::new(Mutex::new(conn)),
            database: database.to_string(),
            version: format!("SQLite {version}"),
        })
    }

    /// Roda o trabalho bloqueante fora do executor do tokio. Uma interrupção
    /// pedida pelo canceller vira [`Error::Cancelled`].
    async fn with_conn<T, F>(&self, work: F) -> Result<T>
    where
        F: FnOnce(&Connection, &str) -> rusqlite::Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let conn = Arc::clone(&self.conn);
        let database = self.database.clone();
        let result = tokio::task::spawn_blocking(move || work(&conn.lock(), &database))
            .await
            .map_err(|e| Error::InvalidState(format!("SQLite task failed: {e}")))?;

        result.map_err(|error| match error.sqlite_error_code() {
            Some(rusqlite::ErrorCode::OperationInterrupted) => Error::Cancelled,
            _ => db_err(error),
        })
    }
}

/// `schema_version` e as relações do banco, numa leitura só.
fn read_tables(conn: &Connection, database: &str) -> rusqlite::Result<(i64, Vec<TableRecord>)> {
    let schema = quote_ident(database);
    let version: i64 = conn.query_row(&format!("PRAGMA {schema}.schema_version"), [], |row| row.get(0))?;

    // As internas (`sqlite_sequence`, `sqlite_stat1`…) ficam de fora, como na
    // listagem do adapter
    let mut stmt = conn.prepare(&format!(
        "SELECT name, type FROM {schema}.sqlite_schema \
         WHERE type IN ('table', 'view') AND name NOT LIKE 'sqlite_%' \
         ORDER BY name"
    ))?;
    let tables = stmt
        .query_map([], |row| {
            let kind: String = row.get(1)?;
            Ok(TableRecord {
                name: row.get(0)?,
                kind: if kind == "view" { RelKind::View } else { RelKind::Table },
                partition_of: None,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    Ok((version, tables))
}

/// Páginas da tabela e dos índices dela (`dbstat`, compilado no SQLite do
/// app). View não tem páginas: `None`.
fn table_size(conn: &Connection, database: &str, table: &str) -> rusqlite::Result<Option<i64>> {
    let schema = quote_ident(database);
    conn.query_row(
        &format!(
            "SELECT sum(pgsize) FROM dbstat(?1) WHERE name IN (
                SELECT name FROM {schema}.sqlite_schema
                WHERE tbl_name = ?2 AND type IN ('table', 'index'))"
        ),
        [database, table],
        |row| row.get::<_, Option<i64>>(0),
    )
    .optional()
    .map(Option::flatten)
}

/// Todas as chaves estrangeiras do arquivo, numa consulta (`pragma_foreign_key_list`
/// de cada tabela). Banco local e pequeno: filtrar depois sai mais barato que
/// uma consulta por tabela. `to` nulo = a chave primária da referenciada.
fn read_all_foreign_keys(conn: &Connection, database: &str) -> rusqlite::Result<Vec<ForeignKey>> {
    let schema = quote_ident(database);
    let mut stmt = conn.prepare(&format!(
        "SELECT m.name, f.id, f.\"table\", f.\"from\", f.\"to\"
         FROM {schema}.sqlite_schema AS m, pragma_foreign_key_list(m.name, ?1) AS f
         WHERE m.type = 'table' AND m.name NOT LIKE 'sqlite_%'
         ORDER BY m.name, f.id, f.seq"
    ))?;
    let rows = stmt.query_map([database], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    })?;

    let mut keys: Vec<ForeignKey> = Vec::new();
    let mut current: Option<(String, i64)> = None;
    let mut missing_to = false;
    for row in rows {
        let (table, id, ref_table, from, to) = row?;
        if current.as_ref() != Some(&(table.clone(), id)) {
            if missing_to {
                if let Some(last) = keys.last_mut() {
                    last.ref_columns.clear();
                }
            }
            missing_to = false;
            current = Some((table.clone(), id));
            keys.push(ForeignKey {
                schema: FLAT_SCHEMA.to_string(),
                table,
                columns: Vec::new(),
                ref_schema: FLAT_SCHEMA.to_string(),
                ref_table,
                ref_columns: Vec::new(),
            });
        }
        let key = keys.last_mut().expect("chave aberta acima");
        key.columns.push(from);
        match to {
            Some(to) => key.ref_columns.push(to),
            None => missing_to = true,
        }
    }
    if missing_to {
        if let Some(last) = keys.last_mut() {
            last.ref_columns.clear();
        }
    }
    Ok(keys)
}

/// Colunas de cada relação pedida (`pragma_table_info`), na ordem pedida. O
/// SQLite não tem comentários.
fn read_details(conn: &Connection, database: &str, tables: &[String]) -> rusqlite::Result<Vec<RelationDetail>> {
    let mut stmt = conn.prepare(
        "SELECT name, type, \"notnull\", pk FROM pragma_table_info(?1, ?2) ORDER BY cid",
    )?;
    let mut details = Vec::with_capacity(tables.len());
    for table in tables {
        let columns = stmt
            .query_map([table.as_str(), database], |row| {
                Ok(ColumnDetail {
                    name: row.get(0)?,
                    data_type: row.get(1)?,
                    nullable: row.get::<_, i64>(2)? == 0,
                    primary_key: row.get::<_, i64>(3)? > 0,
                    comment: None,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        if !columns.is_empty() {
            details.push(RelationDetail {
                schema: FLAT_SCHEMA.to_string(),
                table: table.clone(),
                comment: None,
                columns,
            });
        }
    }
    Ok(details)
}

/// Colunas cujo nome casa com o `LIKE` (escape `\\`; o `LIKE` do SQLite não
/// diferencia maiúsculas em ASCII), até `limit`, das tabelas e views do banco.
fn search_columns(
    conn: &Connection,
    database: &str,
    pattern: &str,
    limit: usize,
) -> rusqlite::Result<Vec<ColumnMatch>> {
    let schema = quote_ident(database);
    let mut stmt = conn.prepare(&format!(
        "SELECT m.name, p.name, p.type FROM {schema}.sqlite_schema AS m \
         JOIN pragma_table_info(m.name, ?1) AS p \
         WHERE m.type IN ('table', 'view') AND m.name NOT LIKE 'sqlite_%' \
           AND p.name LIKE ?2 ESCAPE '\\' \
         ORDER BY m.name, p.cid LIMIT ?3"
    ))?;
    let rows = stmt.query_map(rusqlite::params![database, pattern, limit as i64], |row| {
        Ok(ColumnMatch {
            table: row.get(0)?,
            column: row.get(1)?,
            data_type: row.get(2)?,
        })
    })?;
    rows.collect()
}

/// Índices do arquivo (ou de uma tabela), do `sqlite_schema`. Os autoindexes
/// (`sqlite_autoindex_*`, de `UNIQUE`/`PRIMARY KEY`) não têm `sql` e ficam de
/// fora, como os internos.
fn read_indexes(conn: &Connection, database: &str, table: Option<&str>) -> rusqlite::Result<Vec<IndexObject>> {
    let schema = quote_ident(database);
    let mut stmt = conn.prepare(&format!(
        "SELECT m.name, m.tbl_name, il.\"unique\", il.partial, m.sql
         FROM {schema}.sqlite_schema AS m
         JOIN pragma_index_list(m.tbl_name, ?1) AS il ON il.name = m.name
         WHERE m.type = 'index' AND m.sql IS NOT NULL AND m.name NOT LIKE 'sqlite_%'
           AND (?2 IS NULL OR m.tbl_name = ?2)
         ORDER BY m.name"
    ))?;
    let listed = stmt
        .query_map(rusqlite::params![database, table], |row| {
            let partial = row.get::<_, i64>(3)? != 0;
            let sql: String = row.get(4)?;
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)? != 0,
                partial.then(|| partial_predicate(&sql)).flatten(),
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    // Expressão não tem nome de coluna (`name` nulo)
    let mut columns = conn.prepare("SELECT name FROM pragma_index_info(?1, ?2) ORDER BY seqno")?;
    listed
        .into_iter()
        .map(|(name, table, unique, predicate)| {
            let cols = columns
                .query_map([name.as_str(), database], |row| {
                    Ok(row.get::<_, Option<String>>(0)?.unwrap_or_else(|| "(expr)".to_string()))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(IndexObject {
                name,
                table,
                // O SQLite só tem b-tree
                method: "btree".to_string(),
                unique,
                primary: false,
                columns: cols,
                predicate,
            })
        })
        .collect()
}

/// A condição de um índice parcial: o que vem depois do último `WHERE` do
/// `CREATE INDEX` guardado (o SQLite não guarda a condição à parte). Basta
/// para mostrar; a definição inteira vem por `definition`.
fn partial_predicate(sql: &str) -> Option<String> {
    let at = sql.to_ascii_uppercase().rfind("WHERE")?;
    let predicate = sql[at + "WHERE".len()..].trim().trim_end_matches(';').trim();
    (!predicate.is_empty()).then(|| predicate.to_string())
}

/// Triggers do arquivo (ou de uma tabela), do `sqlite_schema`. O momento e os
/// eventos saem do cabeçalho do `CREATE TRIGGER` guardado ([`trigger_header`]).
fn read_triggers(conn: &Connection, database: &str, table: Option<&str>) -> rusqlite::Result<Vec<TriggerObject>> {
    let schema = quote_ident(database);
    let mut stmt = conn.prepare(&format!(
        "SELECT name, tbl_name, sql FROM {schema}.sqlite_schema
         WHERE type = 'trigger' AND (?1 IS NULL OR tbl_name = ?1)
         ORDER BY tbl_name, name"
    ))?;
    let rows = stmt.query_map([table], |row| {
        let sql: Option<String> = row.get(2)?;
        let (timing, events) = trigger_header(sql.as_deref().unwrap_or_default());
        Ok(TriggerObject {
            name: row.get(0)?,
            table: row.get(1)?,
            timing,
            events,
            function: None,
            enabled: true,
        })
    })?;
    rows.collect()
}

/// `CREATE TRIGGER nome [BEFORE|AFTER|INSTEAD OF] INSERT|UPDATE [OF …]|DELETE ON …`:
/// o momento (sem ele, o SQLite assume `BEFORE`) e o evento, do trecho antes
/// do `ON`.
fn trigger_header(sql: &str) -> (String, Vec<String>) {
    let upper = sql.to_ascii_uppercase();
    let words: Vec<&str> = upper.split_whitespace().collect();
    let on = words.iter().position(|word| *word == "ON").unwrap_or(words.len());
    let head = &words[..on];
    let timing = if head.windows(2).any(|pair| pair == ["INSTEAD", "OF"]) {
        "INSTEAD OF"
    } else if head.contains(&"AFTER") {
        "AFTER"
    } else {
        "BEFORE"
    };
    let events = ["INSERT", "UPDATE", "DELETE"]
        .iter()
        .filter(|event| head.contains(event))
        .map(|event| event.to_string())
        .collect();
    (timing.to_string(), events)
}

/// O nome sintético de uma chave estrangeira do SQLite (que não tem nome): a
/// tabela e o `id` do `pragma_foreign_key_list`.
fn foreign_key_name(table: &str, id: i64) -> String {
    format!("fk_{table}_{id}")
}

/// Chaves estrangeiras do arquivo (ou de uma tabela), pelo
/// `pragma_foreign_key_list` de cada tabela, uma linha por coluna da chave.
fn read_foreign_keys(
    conn: &Connection,
    database: &str,
    table: Option<&str>,
) -> rusqlite::Result<Vec<ForeignKeyObject>> {
    let schema = quote_ident(database);
    let mut stmt = conn.prepare(&format!(
        "SELECT m.name, f.id, f.\"table\", f.\"from\", f.\"to\", f.on_delete, f.on_update
         FROM {schema}.sqlite_schema AS m, pragma_foreign_key_list(m.name, ?1) AS f
         WHERE m.type = 'table' AND m.name NOT LIKE 'sqlite_%' AND (?2 IS NULL OR m.name = ?2)
         ORDER BY m.name, f.id, f.seq"
    ))?;
    let rows = stmt.query_map(rusqlite::params![database, table], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, Option<String>>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, String>(6)?,
        ))
    })?;

    let mut keys: Vec<(i64, ForeignKeyObject)> = Vec::new();
    for row in rows {
        let (table, id, ref_table, from, to, on_delete, on_update) = row?;
        let same = keys.last().is_some_and(|(last_id, key)| *last_id == id && key.table == table);
        if !same {
            keys.push((
                id,
                ForeignKeyObject {
                    name: foreign_key_name(&table, id),
                    table,
                    columns: Vec::new(),
                    ref_schema: FLAT_SCHEMA.to_string(),
                    ref_table,
                    ref_columns: Vec::new(),
                    on_delete,
                    on_update,
                },
            ));
        }
        let key = &mut keys.last_mut().expect("chave aberta acima").1;
        key.columns.push(from);
        // `to` nulo = a chave primária da referenciada (fica implícito)
        if let Some(to) = to {
            key.ref_columns.push(to);
        }
    }
    Ok(keys.into_iter().map(|(_, key)| key).collect())
}

/// A cláusula `FOREIGN KEY` de uma chave (o SQLite só a guarda dentro do
/// `CREATE TABLE`), remontada do pragma.
fn foreign_key_definition(key: &ForeignKeyObject) -> String {
    let list = |names: &[String]| names.iter().map(|name| quote_ident(name)).collect::<Vec<_>>().join(", ");
    let to = if key.ref_columns.is_empty() {
        String::new()
    } else {
        format!(" ({})", list(&key.ref_columns))
    };
    format!(
        "-- Dentro do CREATE TABLE {}\nFOREIGN KEY ({}) REFERENCES {}{to}\n  ON DELETE {}\n  ON UPDATE {};\n",
        quote_ident(&key.table),
        list(&key.columns),
        quote_ident(&key.ref_table),
        key.on_delete,
        key.on_update
    )
}

/// O `CREATE` guardado no `sqlite_schema` de uma view ou de um índice.
fn read_definition(conn: &Connection, database: &str, kind: &str, name: &str) -> rusqlite::Result<Option<String>> {
    let schema = quote_ident(database);
    conn.query_row(
        &format!("SELECT sql FROM {schema}.sqlite_schema WHERE type = ?1 AND name = ?2"),
        [kind, name],
        |row| row.get::<_, Option<String>>(0),
    )
    .optional()
    .map(|sql| {
        sql.flatten()
            .map(|sql| format!("{};\n", sql.trim_end().trim_end_matches(';')))
    })
}

struct SqliteCanceller(Arc<InterruptHandle>);

#[async_trait]
impl SourceCanceller for SqliteCanceller {
    async fn cancel(&self) -> Result<()> {
        self.0.interrupt();
        Ok(())
    }
}

#[async_trait]
impl CatalogSource for SqliteSource {
    async fn sync(
        &mut self,
        catalog: &RwLock<Catalog>,
        on_event: &mut (dyn FnMut(SyncEvent) + Send),
    ) -> Result<SyncReport> {
        let started = Instant::now();
        let (version, tables) = self.with_conn(read_tables).await?;
        Ok(sync_flat(catalog, tables, version, started, on_event))
    }

    async fn load_schemas(&mut self, catalog: &RwLock<Catalog>, schemas: &[String]) -> Result<usize> {
        if !schemas.iter().any(|schema| schema == FLAT_SCHEMA) {
            return Ok(0);
        }
        let (version, tables) = self.with_conn(read_tables).await?;
        catalog.write().set_tables(FLAT_SCHEMA, tables, Some(version));
        Ok(1)
    }

    async fn relation_size(&mut self, _schema: &str, table: &str) -> Result<Option<i64>> {
        let table = table.to_string();
        // Sem `dbstat` (SQLite de fora), fica sem tamanho
        Ok(self
            .with_conn(move |conn, database| table_size(conn, database, &table))
            .await
            .unwrap_or(None))
    }

    async fn foreign_keys(
        &mut self,
        scope: &[(String, String)],
        of: &[(String, String)],
    ) -> Result<Vec<ForeignKey>> {
        if scope.is_empty() || of.is_empty() {
            return Ok(Vec::new());
        }
        let all = self.with_conn(read_all_foreign_keys).await?;
        let scope: std::collections::HashSet<&str> = scope.iter().map(|(_, table)| table.as_str()).collect();
        let of: std::collections::HashSet<&str> = of.iter().map(|(_, table)| table.as_str()).collect();
        Ok(all
            .into_iter()
            .filter(|key| scope.contains(key.table.as_str()) && scope.contains(key.ref_table.as_str()))
            .filter(|key| of.contains(key.table.as_str()) || of.contains(key.ref_table.as_str()))
            .collect())
    }

    async fn relation_details(&mut self, relations: &[(String, String)]) -> Result<Vec<RelationDetail>> {
        let tables: Vec<String> = relations
            .iter()
            .filter(|(schema, _)| schema == FLAT_SCHEMA)
            .map(|(_, table)| table.clone())
            .collect();
        self.with_conn(move |conn, database| read_details(conn, database, &tables))
            .await
    }

    async fn search_columns(&mut self, schema: &str, pattern: &str, limit: usize) -> Result<Vec<ColumnMatch>> {
        if schema != FLAT_SCHEMA {
            return Ok(Vec::new());
        }
        let pattern = pattern.to_string();
        self.with_conn(move |conn, database| search_columns(conn, database, &pattern, limit))
            .await
    }

    async fn list_indexes(&mut self, schema: &str, table: Option<&str>) -> Result<Vec<IndexObject>> {
        if schema != FLAT_SCHEMA {
            return Ok(Vec::new());
        }
        let table = table.map(str::to_string);
        self.with_conn(move |conn, database| read_indexes(conn, database, table.as_deref()))
            .await
    }

    async fn list_triggers(&mut self, schema: &str, table: Option<&str>) -> Result<Vec<TriggerObject>> {
        if schema != FLAT_SCHEMA {
            return Ok(Vec::new());
        }
        let table = table.map(str::to_string);
        self.with_conn(move |conn, database| read_triggers(conn, database, table.as_deref()))
            .await
    }

    async fn list_foreign_keys(&mut self, schema: &str, table: Option<&str>) -> Result<Vec<ForeignKeyObject>> {
        if schema != FLAT_SCHEMA {
            return Ok(Vec::new());
        }
        let table = table.map(str::to_string);
        self.with_conn(move |conn, database| read_foreign_keys(conn, database, table.as_deref()))
            .await
    }

    async fn definition(&mut self, target: &ObjectRef) -> Result<Option<String>> {
        if target.schema != FLAT_SCHEMA {
            return Ok(None);
        }
        let kind = match target.kind {
            ObjectKind::View => "view",
            ObjectKind::Index => "index",
            ObjectKind::Trigger => "trigger",
            ObjectKind::ForeignKey => {
                let (Some(table), name) = (target.table.clone(), target.name.clone()) else {
                    return Ok(None);
                };
                let keys = self
                    .with_conn(move |conn, database| read_foreign_keys(conn, database, Some(&table)))
                    .await?;
                return Ok(keys.iter().find(|key| key.name == name).map(foreign_key_definition));
            }
            ObjectKind::MaterializedView | ObjectKind::Function | ObjectKind::Sequence => return Ok(None),
        };
        let name = target.name.clone();
        self.with_conn(move |conn, database| read_definition(conn, database, kind, &name))
            .await
    }

    fn canceller(&self) -> Box<dyn SourceCanceller> {
        Box::new(SqliteCanceller(Arc::clone(&self.interrupt)))
    }

    fn is_closed(&self) -> bool {
        false
    }

    fn server_version(&self) -> String {
        self.version.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{CatalogPath, LoadState};
    use crate::models::DatabaseType;

    fn server(path: &str) -> Server {
        Server {
            id: Some(1),
            name: "notas".into(),
            db_type: DatabaseType::Sqlite,
            host: String::new(),
            port: 0,
            username: String::new(),
            password: String::new(),
            default_database: None,
            ssl_mode: crate::models::SslMode::Disable,
            ssl_root_cert: None,
            connection_uri: Some(path.into()),
            scope_databases: None,
            scope_schemas: None,
            group_id: None,
            assistant_sample_values: false,
            created_at: 0,
        }
    }

    /// Arquivo novo por teste (o adapter não cria banco)
    fn fresh_file(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!("octapus_db_sqlite_catalog_{name}.db"));
        let _ = std::fs::remove_file(&path);
        path
    }

    fn names(catalog: &RwLock<Catalog>) -> Vec<String> {
        catalog
            .read()
            .children(&CatalogPath::Schema(FLAT_SCHEMA.into()), None, 0, 100)
            .unwrap()
            .items
            .into_iter()
            .map(|node| node.name)
            .collect()
    }

    #[tokio::test]
    async fn syncs_tables_and_views_and_follows_schema_changes() {
        let path = fresh_file("sync");
        let writer = Connection::open(&path).unwrap();
        writer
            .execute_batch(
                "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT);
                 CREATE INDEX idx_users_name ON users(name);
                 CREATE TABLE orders (id INTEGER PRIMARY KEY AUTOINCREMENT, user_id INTEGER);
                 CREATE VIEW active AS SELECT * FROM users;
                 INSERT INTO users (name) VALUES ('ana'), ('bia');",
            )
            .unwrap();

        let mut source = SqliteSource::connect(&server(path.to_str().unwrap()), "main").await.unwrap();
        assert!(source.server_version().starts_with("SQLite 3."));

        let catalog = RwLock::new(Catalog::new());
        let mut events = Vec::new();
        let report = source.sync(&catalog, &mut |event| events.push(event)).await.unwrap();
        assert_eq!(report.fetched, 1);
        assert_eq!(events.len(), 2);
        // `sqlite_sequence` (do AUTOINCREMENT) fica de fora
        assert_eq!(names(&catalog), vec!["active", "orders", "users"]);

        // Nada mudou: revalidar não recarrega
        let report = source.sync(&catalog, &mut |_| {}).await.unwrap();
        assert_eq!(report.fetched, 0);

        writer.execute_batch("CREATE TABLE invoices (id INTEGER)").unwrap();
        let report = source.sync(&catalog, &mut |_| {}).await.unwrap();
        assert_eq!(report.diff.changed, vec![FLAT_SCHEMA.to_string()]);
        assert_eq!(names(&catalog), vec!["active", "invoices", "orders", "users"]);
        assert_eq!(catalog.read().schema_state(FLAT_SCHEMA), Some(LoadState::Loaded));

        // Carga prioritária: só o schema sem nome existe
        assert_eq!(source.load_schemas(&catalog, &["outro".into()]).await.unwrap(), 0);
        assert_eq!(source.load_schemas(&catalog, &[FLAT_SCHEMA.into()]).await.unwrap(), 1);

        let size = source.relation_size(FLAT_SCHEMA, "users").await.unwrap();
        assert!(size.is_some_and(|bytes| bytes > 0));
        assert_eq!(source.relation_size(FLAT_SCHEMA, "active").await.unwrap(), None);
    }

    #[tokio::test]
    async fn lists_triggers_and_foreign_keys_with_definitions() {
        let path = fresh_file("triggers");
        Connection::open(&path)
            .unwrap()
            .execute_batch(
                "CREATE TABLE users (id INTEGER PRIMARY KEY, email TEXT);
                 CREATE TABLE orders (id INTEGER PRIMARY KEY, user_id INTEGER REFERENCES users ON DELETE CASCADE,
                                      a INTEGER, b INTEGER,
                                      FOREIGN KEY (a, b) REFERENCES pairs (x, y));
                 CREATE TABLE pairs (x INTEGER, y INTEGER, PRIMARY KEY (x, y));
                 CREATE TABLE audit (msg TEXT);
                 CREATE TRIGGER orders_log AFTER INSERT ON orders BEGIN INSERT INTO audit VALUES ('x'); END;
                 CREATE TRIGGER users_guard BEFORE UPDATE OF email ON users BEGIN SELECT 1; END;",
            )
            .unwrap();
        let mut source = SqliteSource::connect(&server(path.to_str().unwrap()), "main")
            .await
            .unwrap();

        let triggers = source.list_triggers(FLAT_SCHEMA, None).await.unwrap();
        let summary: Vec<(&str, &str, &str, Vec<String>)> = triggers
            .iter()
            .map(|t| (t.name.as_str(), t.table.as_str(), t.timing.as_str(), t.events.clone()))
            .collect();
        assert_eq!(
            summary,
            [
                ("orders_log", "orders", "AFTER", vec!["INSERT".to_string()]),
                ("users_guard", "users", "BEFORE", vec!["UPDATE".to_string()]),
            ]
        );
        assert_eq!(source.list_triggers(FLAT_SCHEMA, Some("users")).await.unwrap().len(), 1);

        let keys = source.list_foreign_keys(FLAT_SCHEMA, Some("orders")).await.unwrap();
        assert_eq!(keys.len(), 2);
        let to_users = keys.iter().find(|key| key.ref_table == "users").unwrap();
        // Sem coluna referenciada: a chave primária implícita
        assert_eq!(to_users.columns, ["user_id"]);
        assert!(to_users.ref_columns.is_empty());
        assert_eq!(to_users.on_delete, "CASCADE");
        let pair = keys.iter().find(|key| key.ref_table == "pairs").unwrap();
        assert_eq!((pair.columns.as_slice(), pair.ref_columns.as_slice()),
            (["a", "b"].map(String::from).as_slice(), ["x", "y"].map(String::from).as_slice())
        );
        assert!(source.list_foreign_keys(FLAT_SCHEMA, Some("audit")).await.unwrap().is_empty());

        let target = |kind, name: &str, table: Option<&str>| ObjectRef {
            kind,
            schema: FLAT_SCHEMA.into(),
            name: name.into(),
            table: table.map(str::to_string),
            signature: None,
        };
        let trigger = source
            .definition(&target(ObjectKind::Trigger, "orders_log", Some("orders")))
            .await
            .unwrap()
            .unwrap();
        assert!(trigger.starts_with("CREATE TRIGGER orders_log AFTER INSERT ON orders"), "{trigger}");
        let key = source
            .definition(&target(ObjectKind::ForeignKey, &pair.name, Some("orders")))
            .await
            .unwrap()
            .unwrap();
        assert!(
            key.contains("FOREIGN KEY (\"a\", \"b\") REFERENCES \"pairs\" (\"x\", \"y\")"),
            "{key}"
        );
        let missing = target(ObjectKind::ForeignKey, "fk_x_9", Some("orders"));
        assert!(source.definition(&missing).await.unwrap().is_none());
    }

    #[test]
    fn reads_trigger_headers() {
        assert_eq!(
            trigger_header("CREATE TRIGGER t INSTEAD OF DELETE ON v BEGIN SELECT 1; END"),
            ("INSTEAD OF".to_string(), vec!["DELETE".to_string()])
        );
        // Sem momento, BEFORE; o UPDATE OF col conta como UPDATE; o corpo não entra
        assert_eq!(
            trigger_header("create trigger t update of a, b on x begin insert into y values (1); end"),
            ("BEFORE".to_string(), vec!["UPDATE".to_string()])
        );
    }

    #[tokio::test]
    async fn lists_indexes_without_autoindexes_and_reads_definitions() {
        let path = fresh_file("objects");
        Connection::open(&path)
            .unwrap()
            .execute_batch(
                "CREATE TABLE users (id INTEGER PRIMARY KEY, email TEXT UNIQUE, name TEXT);
                 CREATE INDEX idx_users_name ON users(name, lower(email)) WHERE name <> 'x';
                 CREATE UNIQUE INDEX idx_users_email ON users(email);
                 CREATE TABLE orders (id INTEGER PRIMARY KEY, user_id INTEGER);
                 CREATE INDEX idx_orders_user ON orders(user_id);
                 CREATE VIEW active AS SELECT * FROM users;",
            )
            .unwrap();
        let mut source = SqliteSource::connect(&server(path.to_str().unwrap()), "main")
            .await
            .unwrap();

        let all = source.list_indexes(FLAT_SCHEMA, None).await.unwrap();
        let names: Vec<&str> = all.iter().map(|index| index.name.as_str()).collect();
        // O `UNIQUE` de `email` vira `sqlite_autoindex_users_1`: fica de fora
        assert_eq!(names, ["idx_orders_user", "idx_users_email", "idx_users_name"]);
        let by_name = all.iter().find(|index| index.name == "idx_users_name").unwrap();
        assert_eq!((by_name.table.as_str(), by_name.unique), ("users", false));
        assert_eq!(by_name.columns, ["name", "(expr)"]);
        assert_eq!(by_name.predicate.as_deref(), Some("name <> 'x'"));
        assert_eq!(
            all.iter()
                .find(|index| index.name == "idx_users_email")
                .unwrap()
                .predicate,
            None
        );
        assert!(all.iter().find(|index| index.name == "idx_users_email").unwrap().unique);

        let of_orders = source.list_indexes(FLAT_SCHEMA, Some("orders")).await.unwrap();
        assert_eq!(of_orders.len(), 1);

        let target = |kind, name: &str| ObjectRef {
            kind,
            schema: FLAT_SCHEMA.into(),
            name: name.into(),
            table: None,
            signature: None,
        };
        let view = source
            .definition(&target(ObjectKind::View, "active"))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(view, "CREATE VIEW active AS SELECT * FROM users;\n");
        let index = source
            .definition(&target(ObjectKind::Index, "idx_orders_user"))
            .await
            .unwrap()
            .unwrap();
        assert!(index.starts_with("CREATE INDEX idx_orders_user ON orders(user_id)"));
        assert!(source
            .definition(&target(ObjectKind::Index, "nope"))
            .await
            .unwrap()
            .is_none());
        assert!(source
            .definition(&target(ObjectKind::Function, "f"))
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn interrupt_cancels_the_running_read() {
        let path = fresh_file("interrupt");
        Connection::open(&path)
            .unwrap()
            .execute_batch("CREATE TABLE t (id INTEGER)")
            .unwrap();
        let source = SqliteSource::connect(&server(path.to_str().unwrap()), "main").await.unwrap();
        let canceller = source.canceller();

        // Uma leitura sem fim, interrompida de outra task
        let running = source.with_conn(|conn, _| {
            conn.query_row(
                "WITH RECURSIVE n(x) AS (SELECT 1 UNION ALL SELECT x + 1 FROM n) SELECT count(*) FROM n",
                [],
                |row| row.get::<_, i64>(0),
            )
        });
        let cancel = async {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            canceller.cancel().await.unwrap();
        };
        let (result, _) = tokio::join!(running, cancel);
        assert!(matches!(result, Err(Error::Cancelled)));
    }

    #[tokio::test]
    async fn missing_file_fails_to_connect() {
        let error = SqliteSource::connect(&server("/nao/existe.db"), "main").await.err().unwrap();
        assert!(error.to_string().contains("not found"));
    }
}
