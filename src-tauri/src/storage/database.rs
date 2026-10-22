use std::path::Path;

use rusqlite::{Connection, Result};

pub fn init_storage<P: AsRef<Path>>(db_path: P) -> Result<Connection> {
    let conn = Connection::open(db_path)?;

    conn.execute_batch(
        r#"
        PRAGMA journal_mode = WAL;
        PRAGMA foreign_keys = ON;

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

    run_migrations(&conn)?;

    Ok(conn)
}

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