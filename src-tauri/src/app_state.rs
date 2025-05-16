use std::sync::Mutex;
use rusqlite::Connection;

pub struct AppState {
    pub conn: Mutex<Connection>,
}