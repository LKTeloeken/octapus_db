use tauri::State;
use crate::{
    app_state::AppState,
    models::PostgreServer,
};
use rusqlite::params;
use std::time::{SystemTime, UNIX_EPOCH};

#[tauri::command]
pub fn create_server(
    state: State<AppState>,
    name: String,
    host: String,
    port: i32,
    username: String,
    password: String,
) -> Result<PostgreServer, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    
    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs() as i64;
    
    let mut stmt = conn.prepare(
        r#"
        INSERT INTO servers (name, host, port, username, password, created_at) 
        VALUES (?1, ?2, ?3, ?4, ?5, ?6) 
        RETURNING id, name, host, port, username, password, created_at
        "#
    ).map_err(|e| e.to_string())?;
    
    stmt.query_row(params![name, host, port, username, password, created_at], |row| {
        Ok(PostgreServer {
            id: Some(row.get(0)?),
            name: row.get(1)?,
            host: row.get(2)?,
            port: row.get(3)?,
            username: row.get(4)?,
            password: row.get(5)?,
            created_at: row.get(6)?,
        })
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_all_servers(state: State<AppState>) -> Result<Vec<PostgreServer>, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    
    let mut stmt = conn.prepare(
        "SELECT id, name, host, port, username, password, created_at FROM servers ORDER BY name"
    ).map_err(|e| e.to_string())?;
    
    let rows = stmt.query_map([], |row| {
        Ok(PostgreServer {
            id: Some(row.get(0)?),
            name: row.get(1)?,
            host: row.get(2)?,
            port: row.get(3)?,
            username: row.get(4)?,
            password: row.get(5)?,
            created_at: row.get(6)?,
        })
    }).map_err(|e| e.to_string())?;
    
    let mut servers = Vec::new();
    
    for row in rows {
        match row {
            Ok(server) => servers.push(server),
            Err(e) => return Err(e.to_string()),
        }
    }
    
    Ok(servers)
}

#[tauri::command]
pub fn get_server_by_id(state: State<AppState>, id: i32) -> Result<PostgreServer, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    
    let mut stmt = conn.prepare(
        "SELECT id, name, host, port, username, password, created_at FROM servers WHERE id = ?"
    ).map_err(|e| e.to_string())?;
    
    let server = stmt.query_row(params![id], |row| {
        Ok(PostgreServer {
            id: Some(row.get(0)?),
            name: row.get(1)?,
            host: row.get(2)?,
            port: row.get(3)?,
            username: row.get(4)?,
            password: row.get(5)?,
            created_at: row.get(6)?,
        })
    }).map_err(|e| e.to_string())?;
    
    Ok(server)
}

#[tauri::command]
pub fn update_server(
    state: State<AppState>,
    id: i32,
    name: String,
    host: String,
    port: i32,
    username: String,
    password: String,
) -> Result<PostgreServer, String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    
    let mut stmt = conn.prepare(
        r#"
        UPDATE servers 
        SET name = ?1, host = ?2, port = ?3, username = ?4, password = ?5 
        WHERE id = ?6
        RETURNING id, name, host, port, username, password, created_at
        "#
    ).map_err(|e| e.to_string())?;
    
    stmt.query_row(params![name, host, port, username, password, id], |row| {
        Ok(PostgreServer {
            id: Some(row.get(0)?),
            name: row.get(1)?,
            host: row.get(2)?,
            port: row.get(3)?,
            username: row.get(4)?,
            password: row.get(5)?,
            created_at: row.get(6)?,
        })
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_server(state: State<AppState>, id: i32) -> Result<(), String> {
    let conn = state.conn.lock().map_err(|e| e.to_string())?;
    
    let result = conn.execute(
        "DELETE FROM servers WHERE id = ?",
        params![id],
    ).map_err(|e| e.to_string())?;
    
    if result == 0 {
        return Err("Server not found".to_string());
    }
    
    Ok(())
}

