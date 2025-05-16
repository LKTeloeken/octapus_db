// filepath: /Users/lucasteloeken/Documents/projects/octapus_db/src-tauri/src/app_state.rs
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

pub struct AppState {
    pub conn: Arc<Mutex<Connection>>,
}

impl AppState {
    pub fn new(db_path: &str) -> Result<Self, String> {
        let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
        Ok(AppState {
            conn: Arc::new(Mutex::new(conn)),
        })
    }
}