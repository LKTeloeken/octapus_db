// filepath: /Users/lucasteloeken/Documents/projects/octapus_db/src-tauri/src/models/connection.rs
#[derive(Debug)]
pub struct DatabaseConnection {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub database_name: String,
}

impl DatabaseConnection {
    pub fn new(host: String, port: u16, username: String, password: String, database_name: String) -> Self {
        DatabaseConnection {
            host,
            port,
            username,
            password,
            database_name,
        }
    }
}