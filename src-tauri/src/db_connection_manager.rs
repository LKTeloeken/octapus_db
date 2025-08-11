use std::collections::HashMap;
use std::time::Duration;
use std::sync::{Arc, Mutex};

use once_cell::sync::Lazy;
use rusqlite::Connection as SqliteConnection;
use postgres::{Client as PgClient, NoTls, Config as PgConfig};

use crate::models::PostgreServer;
use crate::app_state::AppState;

use anyhow::{Result, anyhow};

/// Conexão aberta (Postgres ou SQLite)
pub enum DbConnection {
    Sqlite(SqliteConnection),
    Postgres(PgClient),
}

/// { server_id => { db_name => conexão } }
static CONNECTION_MANAGER: Lazy<Arc<Mutex<HashMap<i32, HashMap<String, DbConnection>>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));

/// Lê credenciais do servidor no SQLite local
fn get_server_config(state: &AppState, id: i32) -> Result<PostgreServer> {
    let conn = state.conn.lock().map_err(|e| anyhow!(e.to_string()))?;
    let mut stmt = conn.prepare(
        "SELECT id, name, host, port, username, password, default_database, created_at
         FROM servers WHERE id = ?",
    )?;

    let server = stmt.query_row([id], |row| {
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
    })?;

    Ok(server)
}

/// Conecta ao Postgres com timeout (roda em thread de bloqueio)
async fn connect_to_postgres_with_timeout(server: &PostgreServer, db_name: &str) -> Result<PgClient> {
    let host = server.host.clone();
    let user = server.username.clone();
    let pass = server.password.clone();
    let port_u16: u16 = server.port.try_into().unwrap_or(5432);
    let db = db_name.to_string();

    // Executa a conexão em pool de tarefas bloqueantes do Tauri
    let client = tauri::async_runtime::spawn_blocking(move || {
        let mut cfg = PgConfig::new();
        cfg.host(&host)
            .port(port_u16)
            .user(&user)
            .password(&pass)
            .dbname(&db)
            // timeout real para handshake/DNS (aplicado por endereço)
            .connect_timeout(Duration::from_secs(5)); // ajuste se quiser

        cfg.connect(NoTls).map_err(|e| anyhow!("Erro ao conectar: {e}"))
    })
    .await
    .map_err(|e| anyhow!("JoinError conectando: {e}"))??;

    Ok(client)
}

/// Obtém (ou cria) a conexão {server_id, db_name} de forma assíncrona,
/// sem segurar o mutex durante a operação bloqueante.
pub async fn connect(
    state: &AppState,
    server_id: i32,
    database_name: Option<String>,
) -> Result<()> {
    // 1) Lê config fora de qualquer lock
    let server_cfg = get_server_config(state, server_id)?;

    // 2) Decide o db alvo
    let db_name = database_name
        .unwrap_or_else(|| server_cfg
            .default_database
            .clone()
            .unwrap_or_else(|| "postgres".to_string()));

    // 3) Checa se já existe conexão (lock curto)
    {
        let manager = CONNECTION_MANAGER.lock().unwrap();
        if let Some(db_map) = manager.get(&server_id) {
            if db_map.contains_key(&db_name) {
                // Já existe, nada a fazer
                return Ok(());
            }
        }
    }

    // 4) Conecta sem segurar o lock
    let client = connect_to_postgres_with_timeout(&server_cfg, &db_name).await?;

    // 5) Insere no manager (lock curto) — trata corrida com outro thread
    let mut manager = CONNECTION_MANAGER.lock().unwrap();
    let db_map = manager.entry(server_id).or_insert_with(HashMap::new);
    db_map.entry(db_name.clone())
        .or_insert_with(|| {
            println!("🟢 Nova conexão criada | server_id={} | db={}", server_id, db_name);
            DbConnection::Postgres(client)
        });

    Ok(())
}

/// Executa query e retorna Vec<HashMap<col, val>>.
/// Tira a conexão do mapa, executa em thread bloqueante, e devolve a conexão ao mapa.
pub async fn execute_query(
    state: &AppState,
    server_id: i32,
    query: &str,
    database_name: Option<String>,
) -> Result<Vec<HashMap<String, String>>> {
    // Garante conexão criada
    connect(state, server_id, database_name.clone()).await?;
    let query_owned = query.to_string();

    // remove a conexão do mapa para usá-la no spawn_blocking
    let (db_name, conn) = {
        let mut manager = CONNECTION_MANAGER.lock().unwrap();
        let db_name = database_name.clone().unwrap_or_else(|| "postgres".to_string());
        let db_map = manager.get_mut(&server_id)
            .ok_or_else(|| anyhow!("Servidor {} não encontrado no manager", server_id))?;

        let conn = db_map.remove(&db_name)
            .ok_or_else(|| anyhow!("Conexão não encontrada para db {}", db_name))?;

        (db_name, conn)
    };

    // Executa a consulta fora do runtime principal
    let (conn_back, results) = tauri::async_runtime::spawn_blocking(move || -> Result<(DbConnection, Vec<HashMap<String, String>>)> {
        match conn {
            DbConnection::Sqlite(mut sqlite_conn) => {
                let results = {
                    let mut stmt = sqlite_conn.prepare(&query_owned)?;
                    let col_count = stmt.column_count();
                    let col_names: Vec<String> = (0..col_count)
                        .map(|i| stmt.column_name(i).unwrap_or("unknown").to_string())
                        .collect();

                    let mut rows = stmt.query([])?;
                    let mut results = Vec::new();
                    while let Some(row) = rows.next()? {
                        let mut map = HashMap::new();
                        for i in 0..col_count {
                            let v: rusqlite::Result<String, _> = row.get(i);
                            map.insert(col_names[i].clone(), v.unwrap_or_else(|_| "NULL".into()));
                        }
                        results.push(map);
                    }
                    results
                }; // `stmt` and `rows` dropped here before we move `sqlite_conn`
                Ok((DbConnection::Sqlite(sqlite_conn), results))
            }
            DbConnection::Postgres(mut pg_client) => {
                let rows = pg_client.query(&query_owned, &[])
                    .map_err(|e| anyhow!("Erro na query: {e}"))?;

                let mut results = Vec::new();
                for row in rows {
                    let mut map = HashMap::new();
                    for (i, col) in row.columns().iter().enumerate() {
                        let v: String = row.try_get(i).unwrap_or_else(|_| "NULL".to_string());
                        map.insert(col.name().to_string(), v);
                    }
                    results.push(map);
                }
                Ok((DbConnection::Postgres(pg_client), results))
            }
        }
    })
    .await
    .map_err(|e| anyhow!("JoinError query: {e}"))??;

    // Devolve a conexão ao mapa
    {
        let mut manager = CONNECTION_MANAGER.lock().unwrap();
        let db_map = manager.entry(server_id).or_insert_with(HashMap::new);
        db_map.insert(db_name.clone(), conn_back);
    }

    Ok(results)
}

/// Fecha uma conexão (ou todas)
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
                manager.remove(&server_id);
                println!("🔴 Todas as conexões fechadas para server_id={}", server_id);
            }
            Ok(())
        }
    }
}