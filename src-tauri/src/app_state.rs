use std::sync::Mutex;
use rusqlite::Connection;

pub struct AppState {
    // pub db_conn: Mutex<Connection>,
    pub conn: Mutex<Connection>
}