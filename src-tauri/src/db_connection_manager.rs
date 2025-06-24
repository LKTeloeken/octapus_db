use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use once_cell::sync::Lazy;
use rusqlite::Connection as SqliteConnection;
use postgres::{Client as PgClient, NoTls};

use crate::models::PostgreServer;
use crate::app_state::AppState;

use anyhow::{Result, anyhow};

/// Uma conexão aberta (Postgres ou SQLite)
pub enum DbConnection {
    Sqlite(SqliteConnection),
    Postgres(PgClient),
}

/// type alias apenas para clareza
type DatabaseMap = HashMap<String, DbConnection>;

/// Gerenciador global de conexões
/// { id-servidor => { nome_database => conexão } }
static CONNECTION_MANAGER: Lazy<Arc<Mutex<HashMap<i32, DatabaseMap>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));

/// Lê as credenciais do servidor salvo no SQLite local
fn get_server_config(state: &AppState, id: i32) -> Result<PostgreServer> {
    let conn = state.conn.lock().map_err(|e| anyhow!(e.to_string()))?;

    let mut stmt = conn.prepare(
        "SELECT id, name, host, port, username, password, default_database, created_at
         FROM servers WHERE id = ?",
    )?;

    let server = stmt
        .query_row([id], |row| {
            Ok(PostgreServer {
                id: Some(row.get(0)?),
                name: row.get(1)?,
                host: row.get(2)?,
                port: row.get(3)?,
                username: row.get(4)?,
                password: row.get(5)?,
                default_database: row.get(6)?,
                created_at: row.get(7)?,
            })
        })
        .map_err(|e| anyhow!(e.to_string()))?;

    Ok(server)
}

/// Abre efetivamente uma conexão Postgres para o *database* informado
fn connect_to_postgres(server: &PostgreServer, db_name: &str) -> Result<DbConnection> {
    let conn_str = format!(
        "host={} port={} user={} password={} dbname={}",
        server.host,
        server.port,
        server.username,
        server.password,
        db_name
    );

    let client =
        PgClient::connect(&conn_str, NoTls).map_err(|e| anyhow!("Erro ao conectar: {}", e))?;

    Ok(DbConnection::Postgres(client))
}

/// Cria (ou reutiliza) a conexão {server_id, db_name}
fn get_or_create_connection(
    state: &AppState,
    server_id: i32,
    database_name: Option<String>,
) -> Result<()> {
    // 1 ─ Carrega config do servidor fora do lock
    let server_cfg = get_server_config(state, server_id)?;

    // 2 ─ Define qual database usar
    let db_name = database_name
        .unwrap_or_else(|| server_cfg
            .default_database
            .clone()
            .unwrap_or_else(|| "postgres".to_string()));

    // 3 ─ Agora trava o mutex e trabalha nele
    let mut manager = CONNECTION_MANAGER.lock().unwrap();
    let db_map = manager.entry(server_id).or_insert_with(HashMap::new);

    // 4 ─ Se ainda não existe conexão para esse db, cria
    if !db_map.contains_key(&db_name) {
        let conn = connect_to_postgres(&server_cfg, &db_name)?;
        db_map.insert(db_name.clone(), conn);
        println!(
            "🟢 Nova conexão criada | server_id={} | db={}",
            server_id, db_name
        );
    } else {
        println!(
            "♻️  Reutilizando conexão existente | server_id={} | db={}",
            server_id, db_name
        );
    }
    Ok(())
}

/// === API PÚBLICA ===========================================================

/// Apenas abre (ou reutiliza) a conexão; não executa queries
pub fn connect(
    state: &AppState,
    server_id: i32,
    database_name: Option<String>,
) -> Result<()> {
    get_or_create_connection(state, server_id, database_name)
}

/// Executa query (DDL/DML) e devolve linhas como Vec<HashMap<col, val>>
pub fn execute_query(
    state: &AppState,
    server_id: i32,
    query: &str,
    database_name: Option<String>,
) -> Result<Vec<HashMap<String, String>>> {
    // garante conexão
    connect(state, server_id, database_name.clone())?;

    let mut manager = CONNECTION_MANAGER.lock().unwrap();
    let db_map = manager
        .get_mut(&server_id)
        .ok_or_else(|| anyhow!("Servidor {} não encontrado no manager", server_id))?;

    // determina nome do database correto
    let db_name = database_name.unwrap_or_else(|| "postgres".to_string());
    let conn = db_map
        .get_mut(&db_name)
        .ok_or_else(|| anyhow!("Conexão não encontrada para db {}", db_name))?;

    match conn {
        DbConnection::Sqlite(sqlite_conn) => {
            let mut stmt = sqlite_conn.prepare(query)?;
            let col_count = stmt.column_count();
            let col_names: Vec<String> = (0..col_count)
                .map(|i| stmt.column_name(i).unwrap_or("unknown").to_string())
                .collect();

            let mut rows = stmt.query([])?;
            let mut results = Vec::new();

            while let Some(row) = rows.next()? {
                let mut map = HashMap::new();
                for i in 0..col_count {
                    let v: Result<String, _> = row.get(i);
                    map.insert(col_names[i].clone(), v.unwrap_or_else(|_| "NULL".into()));
                }
                results.push(map);
            }
            Ok(results)
        }
        DbConnection::Postgres(pg_client) => {
            let rows = pg_client.query(query, &[])?;
            let mut results = Vec::new();
            for row in rows {
                let mut map = HashMap::new();
                for (i, col) in row.columns().iter().enumerate() {
                    let v: String =
                        row.try_get(i).unwrap_or_else(|_| "NULL".to_string());
                    map.insert(col.name().to_string(), v);
                }
                results.push(map);
            }
            Ok(results)
        }
    }
}

/// Encerra conexão.
/// Se `db_name` = None ⇒ remove todas as conexões daquele servidor.
pub fn disconnect(server_id: i32, db_name: Option<String>) -> Result<()> {
    let mut manager = CONNECTION_MANAGER.lock().unwrap();
    match manager.get_mut(&server_id) {
        None => Err(anyhow!("Nenhuma conexão para server_id {}", server_id)),
        Some(db_map) => {
            if let Some(name) = db_name {
                if db_map.remove(&name).is_some() {
                    println!("🔴 Conexão fechada | server_id={} | db={}", server_id, name);
                }
            } else {
                // remove todas
                manager.remove(&server_id);
                println!("🔴 Todas as conexões fechadas para server_id={}", server_id);
            }
            Ok(())
        }
    }
}