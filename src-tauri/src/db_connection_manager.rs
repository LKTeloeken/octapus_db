use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use once_cell::sync::Lazy;
use rusqlite::{Connection as SqliteConnection};
use postgres::{Client as PgClient, NoTls};

use crate::models::PostgreServer;
use crate::app_state::AppState;

use anyhow::{Result, anyhow};

/// Enum para armazenar uma conexão aberta (Postgres ou SQLite)
pub enum DbConnection {
    Sqlite(SqliteConnection),
    Postgres(PgClient),
}

/// Gerenciador global de conexões ativas.
/// Usa Arc<Mutex<...>> para thread safety.
static CONNECTION_MANAGER: Lazy<Arc<Mutex<HashMap<i32, DbConnection>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));

/// Recupera a configuração do servidor a partir do id no SQLite local (usando a AppState).
fn get_server_config(state: &AppState, id: i32) -> Result<PostgreServer> {
    let conn = state.conn.lock().map_err(|e| anyhow!(e.to_string()))?;
    
    let mut stmt = conn.prepare(
        "SELECT id, name, host, port, username, password, default_database, created_at FROM servers WHERE id = ?"
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
    }).map_err(|e| anyhow!(e.to_string()))?;
    
    Ok(server)
}

/// Cria uma conexão nova de acordo com as credenciais
fn connect_to_server(server: &PostgreServer) -> Result<DbConnection> {
    let conn_str = format!(
        "host={} port={} user={} password={} dbname={}",
        server.host,
        server.port,
        server.username,
        server.password,
        server.default_database.as_deref().unwrap_or("postgres") 
    );
    
    let client = PgClient::connect(&conn_str, NoTls)
        .map_err(|e| anyhow!("Erro ao conectar no Postgres: {}", e))?;
    
    Ok(DbConnection::Postgres(client))
}

/// Cria ou reusa a conexão para o id do servidor
fn get_or_create_connection(state: &AppState, server_id: i32, database_name: Option<String>) -> Result<()> {
    let mut manager = CONNECTION_MANAGER.lock().unwrap();

    if !manager.contains_key(&server_id) {
        let server_config = get_server_config(state, server_id)?;
        let conn = connect_to_server(&server_config)?;
        manager.insert(server_id, conn);
        println!("🟢 Nova conexão criada para servidor id: {}", server_id);
    } else {
        println!("♻️ Reutilizando conexão existente para servidor id: {}", server_id);
    }

    Ok(())
}

/// Executa uma query simples (DDL/DML) na conexão já aberta
pub fn execute_query(state: &AppState, server_id: i32, query: &str, database_name: Option<String>) -> Result<Vec<HashMap<String, String>>> {
    
    // Garante que a conexão foi criada ou reutilizada
    let _ = get_or_create_connection(state, server_id, database_name).map_err(|e| e.to_string());

    let mut manager = CONNECTION_MANAGER.lock().unwrap();

    let conn = manager.get_mut(&server_id)
        .ok_or_else(|| anyhow!("Conexão não encontrada para id {}", server_id))?;

    match conn {
        DbConnection::Sqlite(sqlite_conn) => {
            let mut stmt = sqlite_conn.prepare(query)?;
            let column_count = stmt.column_count();
            
            // Get column names before executing the query
            let column_names: Vec<String> = (0..column_count)
                .map(|i| stmt.column_name(i).unwrap_or("unknown").to_string())
                .collect();
                
            let mut rows = stmt.query([])?;
            let mut results = Vec::new();

            while let Some(row) = rows.next()? {
                let mut map = HashMap::new();
                for i in 0..column_count {
                    let col_name = &column_names[i];
                    let val: Result<String, _> = row.get(i);
                    map.insert(col_name.clone(), val.unwrap_or("NULL".to_string()));
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
                    let val: String = row
                        .try_get(i)
                        .map(|v: String| v)
                        .unwrap_or_else(|_| "NULL".to_string());
                    map.insert(col.name().to_string(), val);
                }
                results.push(map);
            }

            Ok(results)
        }
    }
}

/// Opcional: encerra a conexão explicitamente (remove do manager)
pub fn disconnect(server_id: i32) -> Result<()> {
    let mut manager = CONNECTION_MANAGER.lock().unwrap();
    if manager.remove(&server_id).is_some() {
        println!("🔴 Conexão encerrada para servidor id: {}", server_id);
        Ok(())
    } else {
        Err(anyhow!("Nenhuma conexão ativa encontrada para id {}", server_id))
    }
}