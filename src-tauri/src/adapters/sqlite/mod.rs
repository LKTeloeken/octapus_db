mod browse;
mod executor;
mod metadata;
mod util;

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::Mutex;
use rusqlite::{Connection, OpenFlags};

use crate::adapters::DatabaseAdapter;
use crate::error::{Error, Result};
use crate::models::*;

pub struct SqliteAdapter {
    /// O rusqlite é bloqueante e a `Connection` não é `Sync`. O Mutex só é
    /// trancado dentro do `spawn_blocking`, nunca atravessando um `await`.
    conn: Arc<Mutex<Connection>>,
    /// Nome do banco (`main`). O SQLite não tem nível de schema, então ele faz
    /// o papel de "schema" nos modelos — mesma convenção do Mongo e do Redis.
    database: String,
}

impl SqliteAdapter {
    pub fn new(server: &Server, database: &str) -> Result<Self> {
        let conn = open(&resolve_path(server)?)?;

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
            database: database.to_string(),
        })
    }

    /// Roda o trabalho bloqueante fora do executor do tokio.
    async fn with_conn<T, F>(&self, work: F) -> Result<T>
    where
        F: FnOnce(&mut Connection) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let conn = Arc::clone(&self.conn);

        tokio::task::spawn_blocking(move || work(&mut conn.lock()))
            .await
            .map_err(|e| Error::InvalidState(format!("SQLite task failed: {e}")))?
    }
}

/// O caminho do arquivo vem do campo de URI de conexão: o SQLite não tem
/// host/porta/usuário, então é o único campo que o formulário preenche.
fn resolve_path(server: &Server) -> Result<String> {
    let raw = server
        .connection_uri
        .as_deref()
        .map(str::trim)
        .filter(|uri| !uri.is_empty())
        .ok_or_else(|| {
            Error::InvalidState("SQLite requires the path to the database file".into())
        })?;

    let path = raw
        .strip_prefix("sqlite://")
        .or_else(|| raw.strip_prefix("sqlite:"))
        .unwrap_or(raw);

    Ok(expand_home(path))
}

/// `~/dados/app.db` é como o caminho costuma ser digitado à mão.
fn expand_home(path: &str) -> String {
    let Some(rest) = path.strip_prefix("~/") else {
        return path.to_string();
    };

    match std::env::var("HOME") {
        Ok(home) => format!("{home}/{rest}"),
        Err(_) => path.to_string(),
    }
}

fn open(path: &str) -> Result<Connection> {
    let in_memory = path == ":memory:" || path.starts_with("file::memory:");

    // Abrir sem SQLITE_OPEN_CREATE é de propósito: um caminho errado tem que
    // falhar, não virar silenciosamente um banco novo e vazio. O banco em
    // memória é a exceção — ele só existe se for criado na abertura.
    if !in_memory && !Path::new(path).exists() {
        return Err(Error::Connection(format!(
            "Database file not found: {path}"
        )));
    }

    let flags = if in_memory {
        OpenFlags::default()
    } else {
        OpenFlags::default().difference(OpenFlags::SQLITE_OPEN_CREATE)
    };

    let conn = Connection::open_with_flags(path, flags)
        .map_err(|e| Error::Connection(format!("Failed to open {path}: {e}")))?;

    // O busy_timeout evita o "database is locked" na primeira colisão com outro
    // processo escrevendo; as foreign keys vêm desligadas por padrão no SQLite.
    conn.execute_batch("PRAGMA busy_timeout = 5000; PRAGMA foreign_keys = ON;")
        .map_err(|e| Error::Connection(format!("Failed to configure {path}: {e}")))?;

    Ok(conn)
}

#[async_trait]
impl DatabaseAdapter for SqliteAdapter {
    fn capabilities(&self) -> AdapterCapabilities {
        AdapterCapabilities::sqlite()
    }

    async fn execute_query(&self, query: &str, options: QueryOptions) -> Result<QueryResult> {
        let query = query.to_string();
        let database = self.database.clone();

        self.with_conn(move |conn| executor::execute_query(conn, &query, options, &database))
            .await
    }

    async fn fetch_table_data(&self, request: TableDataRequest) -> Result<QueryResult> {
        let database = self.database.clone();

        self.with_conn(move |conn| browse::fetch_table_data(conn, request, &database))
            .await
    }

    async fn apply_row_edits(
        &self,
        editable: &EditableInfo,
        edits: Vec<RowEdit>,
    ) -> Result<StatementResult> {
        let editable = editable.clone();

        self.with_conn(move |conn| executor::apply_row_edits(conn, &editable, edits))
            .await
    }

    async fn insert_rows(
        &self,
        editable: &EditableInfo,
        rows: Vec<RowInsert>,
    ) -> Result<StatementResult> {
        let editable = editable.clone();

        self.with_conn(move |conn| executor::insert_rows(conn, &editable, rows))
            .await
    }

    async fn delete_rows(
        &self,
        editable: &EditableInfo,
        pk_values: Vec<Vec<Option<String>>>,
    ) -> Result<StatementResult> {
        let editable = editable.clone();

        self.with_conn(move |conn| executor::delete_rows(conn, &editable, pk_values))
            .await
    }

    async fn execute_statement(&self, statement: &str) -> Result<StatementResult> {
        let statement = statement.to_string();

        self.with_conn(move |conn| executor::execute_statement(conn, &statement))
            .await
    }

    async fn execute_transaction(
        &self,
        statements: Vec<String>,
    ) -> Result<Vec<StatementResult>> {
        self.with_conn(move |conn| executor::execute_transaction(conn, statements))
            .await
    }

    async fn list_databases(&self) -> Result<Vec<DatabaseInfo>> {
        self.with_conn(move |conn| metadata::list_databases(conn)).await
    }

    async fn list_tables(&self, _schema: &str) -> Result<Vec<TableInfo>> {
        let database = self.database.clone();

        self.with_conn(move |conn| metadata::list_tables(conn, &database))
            .await
    }

    async fn list_columns(&self, _schema: &str, table: &str) -> Result<Vec<ColumnInfo>> {
        let table = table.to_string();

        self.with_conn(move |conn| metadata::list_columns(conn, &table))
            .await
    }

    async fn list_indexes(&self, _schema: &str, table: &str) -> Result<Vec<IndexInfo>> {
        let table = table.to_string();

        self.with_conn(move |conn| metadata::list_indexes(conn, &table))
            .await
    }

    async fn list_schemas_with_tables(&self) -> Result<DatabaseStructure> {
        let database = self.database.clone();

        self.with_conn(move |conn| metadata::list_schemas_with_tables(conn, &database))
            .await
    }

    async fn test_connection(&self) -> Result<()> {
        self.with_conn(move |conn| {
            // Um arquivo que não é banco falha aqui, com a mensagem do SQLite
            conn.query_row("SELECT count(*) FROM sqlite_schema", [], |row| {
                row.get::<_, i64>(0)
            })
            .map_err(util::db_err)?;

            Ok(())
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cada teste tem seu próprio arquivo. O adapter não cria banco, então o
    /// arquivo nasce aqui por uma conexão direta do rusqlite.
    fn fixture(name: &str, schema: &str) -> (Server, std::path::PathBuf) {
        let path = std::env::temp_dir().join(format!("octapus_db_sqlite_{name}.db"));
        let _ = std::fs::remove_file(&path);

        Connection::open(&path).unwrap().execute_batch(schema).unwrap();

        (server_for(&path), path)
    }

    fn server_for(path: &std::path::Path) -> Server {
        Server {
            id: Some(1),
            name: "e2e".into(),
            db_type: DatabaseType::Sqlite,
            // Banco de arquivo: nada de rede, só o caminho na URI
            host: String::new(),
            port: 0,
            username: String::new(),
            password: String::new(),
            default_database: None,
            ssl_enabled: false,
            connection_uri: Some(path.to_string_lossy().into_owned()),
            created_at: 0,
        }
    }

    fn editable_for(table: &str) -> EditableInfo {
        EditableInfo {
            schema: "main".into(),
            table: table.into(),
            primary_key_columns: vec!["id".into()],
            primary_key_column_indices: vec![0],
        }
    }

    #[test]
    fn missing_path_is_rejected() {
        let mut server = server_for(std::path::Path::new("/tmp/qualquer.db"));
        server.connection_uri = None;

        let Err(err) = SqliteAdapter::new(&server, "main") else {
            panic!("sem caminho, abrir o banco tinha que falhar");
        };
        assert!(err.to_string().contains("requires the path"));
    }

    /// Um caminho errado tem que falhar, e não virar um banco vazio novo.
    #[test]
    fn missing_file_is_not_created() {
        let path = std::env::temp_dir().join("octapus_db_sqlite_inexistente.db");
        let _ = std::fs::remove_file(&path);

        let Err(err) = SqliteAdapter::new(&server_for(&path), "main") else {
            panic!("abrir um arquivo inexistente tinha que falhar");
        };

        assert!(err.to_string().contains("not found"), "erro inesperado: {err}");
        assert!(!path.exists(), "o adapter não pode criar o arquivo do banco");
    }

    #[test]
    fn sqlite_prefix_and_home_are_normalized() {
        let mut server = server_for(std::path::Path::new("/dados/app.db"));

        server.connection_uri = Some("sqlite:///dados/app.db".into());
        assert_eq!(resolve_path(&server).unwrap(), "/dados/app.db");

        server.connection_uri = Some("  /dados/app.db  ".into());
        assert_eq!(resolve_path(&server).unwrap(), "/dados/app.db");

        if let Ok(home) = std::env::var("HOME") {
            server.connection_uri = Some("~/app.db".into());
            assert_eq!(resolve_path(&server).unwrap(), format!("{home}/app.db"));
        }
    }

    #[tokio::test]
    async fn e2e_sqlite_adapter() {
        let (server, path) = fixture(
            "browse",
            "CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT, age INTEGER); \
             CREATE INDEX idx_users_name ON users(name);",
        );

        let adapter = SqliteAdapter::new(&server, "main").unwrap();
        adapter.test_connection().await.unwrap();

        adapter
            .execute_statement(
                "INSERT INTO users (id, name, age) \
                 WITH RECURSIVE seq(i) AS (\
                     SELECT 1 UNION ALL SELECT i + 1 FROM seq WHERE i < 120\
                 ) \
                 SELECT i, 'user' || i, i % 50 FROM seq",
            )
            .await
            .unwrap();

        // ── Metadados ────────────────────────────────────────────────────
        let databases = adapter.list_databases().await.unwrap();
        assert!(databases.iter().any(|db| db.name == "main"));

        let tables = adapter.list_tables("main").await.unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].name, "users");
        assert_eq!(tables[0].schema, "main");

        let columns = adapter.list_columns("main", "users").await.unwrap();
        assert_eq!(columns.len(), 3);
        assert!(columns[0].is_primary_key);
        assert_eq!(columns[0].data_type, "INTEGER");
        assert!(columns[1].is_nullable);

        let indexes = adapter.list_indexes("main", "users").await.unwrap();
        assert!(indexes
            .iter()
            .any(|index| index.name == "idx_users_name" && index.columns == vec!["name"]));

        // O SQLite não tem schema: a estrutura vem como um "schema" só, o banco
        let structure = adapter.list_schemas_with_tables().await.unwrap();
        assert_eq!(structure.schemas.len(), 1);
        assert_eq!(structure.schemas[0].name, "main");

        // ── Browse com filtro, ordenação e total ─────────────────────────
        let result = adapter
            .fetch_table_data(TableDataRequest {
                schema: None,
                table: "users".into(),
                where_expr: Some("age >= 10".into()),
                sort: vec![SortSpec {
                    column: "id".into(),
                    direction: SortDirection::Desc,
                }],
                limit: 20,
                offset: 0,
                count_total: true,
            })
            .await
            .unwrap();

        assert_eq!(result.row_count, 20);
        assert!(result.has_more);
        // i em 1..=120 com i % 50 >= 10 → 40 + 40 + 11 linhas
        assert_eq!(result.total_count, Some(91));
        // Ordenado por id DESC → a primeira linha é a 120 (age 20, passa no filtro)
        assert_eq!(result.rows[0][0].as_deref(), Some("120"));

        let editable = result.editable_info.expect("tabela com PK é editável");
        assert_eq!(editable.primary_key_columns, vec!["id".to_string()]);
        assert_eq!(editable.schema, "main");

        // Emendar outro comando no filtro tem que ser recusado
        let err = adapter
            .fetch_table_data(TableDataRequest {
                schema: None,
                table: "users".into(),
                where_expr: Some("id = 1 UNION SELECT 1".into()),
                sort: vec![],
                limit: 10,
                offset: 0,
                count_total: false,
            })
            .await
            .unwrap_err();
        assert!(err.to_string().contains("single WHERE expression"));

        // ── Editor livre ─────────────────────────────────────────────────
        let query = adapter
            .execute_query(
                "SELECT * FROM users ORDER BY id",
                QueryOptions {
                    limit: 10,
                    count_total: true,
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        assert_eq!(query.row_count, 10);
        assert!(query.has_more);
        assert_eq!(query.total_count, Some(120));
        assert_eq!(query.columns[0].type_name, "INTEGER");
        assert!(query.editable_info.is_some(), "select de uma tabela é editável");

        // Agregação não corresponde a linhas físicas: não pode virar grade editável
        let aggregate = adapter
            .execute_query("SELECT COUNT(*) AS total FROM users", QueryOptions::default())
            .await
            .unwrap();
        assert_eq!(aggregate.rows[0][0].as_deref(), Some("120"));
        assert!(aggregate.editable_info.is_none());

        // PRAGMA devolve linhas mesmo sem poder ser paginado
        let pragma = adapter
            .execute_query("PRAGMA table_info(users)", QueryOptions::default())
            .await
            .unwrap();
        assert_eq!(pragma.row_count, 3);

        drop(adapter);
        std::fs::remove_file(&path).unwrap();
    }

    /// Protege a coerção por afinidade: sem ela, um número editado na grade
    /// voltaria como texto e quebraria ordenação, comparação e índice.
    #[tokio::test]
    async fn e2e_sqlite_row_crud() {
        let (server, path) = fixture(
            "crud",
            "CREATE TABLE produtos (\
                id INTEGER PRIMARY KEY, \
                nome TEXT, \
                preco REAL, \
                ativo BOOLEAN);",
        );

        let adapter = SqliteAdapter::new(&server, "main").unwrap();
        let editable = editable_for("produtos");

        // ── INSERT ───────────────────────────────────────────────────────
        let inserted = adapter
            .insert_rows(
                &editable,
                vec![RowInsert {
                    values: vec![
                        ("id".into(), Some("1".into())),
                        ("nome".into(), Some("Café".into())),
                        ("preco".into(), Some("19.90".into())),
                        ("ativo".into(), Some("true".into())),
                    ],
                }],
            )
            .await
            .unwrap();
        assert_eq!(inserted.affected_rows, 1);

        // Cada valor tem que ter sido gravado na classe de armazenamento certa
        let types = adapter
            .execute_query(
                "SELECT typeof(id), typeof(nome), typeof(preco), typeof(ativo) FROM produtos",
                QueryOptions::default(),
            )
            .await
            .unwrap();
        assert_eq!(
            types.rows[0]
                .iter()
                .map(|cell| cell.as_deref().unwrap_or(""))
                .collect::<Vec<_>>(),
            vec!["integer", "text", "real", "integer"],
        );

        // ── UPDATE ───────────────────────────────────────────────────────
        let updated = adapter
            .apply_row_edits(
                &editable,
                vec![RowEdit {
                    pk_values: vec![Some("1".into())],
                    changes: vec![
                        ("preco".into(), Some("25.5".into())),
                        ("ativo".into(), Some("false".into())),
                        ("nome".into(), None),
                    ],
                }],
            )
            .await
            .unwrap();
        assert_eq!(updated.affected_rows, 1);

        let after = adapter
            .execute_query(
                "SELECT preco, ativo, nome, typeof(preco) FROM produtos WHERE id = 1",
                QueryOptions::default(),
            )
            .await
            .unwrap();
        assert_eq!(after.rows[0][0].as_deref(), Some("25.5"));
        // O checkbox manda "false"; no SQLite booleano é 0
        assert_eq!(after.rows[0][1].as_deref(), Some("0"));
        assert_eq!(after.rows[0][2], None);
        assert_eq!(after.rows[0][3].as_deref(), Some("real"));

        // Valor que não cabe na afinidade é recusado antes de chegar no banco
        let err = adapter
            .apply_row_edits(
                &editable,
                vec![RowEdit {
                    pk_values: vec![Some("1".into())],
                    changes: vec![("id".into(), Some("abc".into()))],
                }],
            )
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not a valid integer"));

        // ── TRANSAÇÃO ────────────────────────────────────────────────────
        let results = adapter
            .execute_transaction(vec![
                "INSERT INTO produtos (id, nome) VALUES (2, 'Chá')".into(),
                "INSERT INTO produtos (id, nome) VALUES (3, 'Suco')".into(),
            ])
            .await
            .unwrap();
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|result| result.affected_rows == 1));

        // ── DELETE ───────────────────────────────────────────────────────
        let deleted = adapter
            .delete_rows(&editable, vec![vec![Some("1".into())], vec![Some("2".into())]])
            .await
            .unwrap();
        assert_eq!(deleted.affected_rows, 2);

        let remaining = adapter
            .fetch_table_data(TableDataRequest {
                schema: None,
                table: "produtos".into(),
                where_expr: None,
                sort: vec![],
                limit: 50,
                offset: 0,
                count_total: true,
            })
            .await
            .unwrap();
        assert_eq!(remaining.total_count, Some(1));
        assert_eq!(remaining.rows[0][0].as_deref(), Some("3"));

        drop(adapter);
        std::fs::remove_file(&path).unwrap();
    }
}
