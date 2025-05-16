use rusqlite::{Connection, Result};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio_postgres::{Client, NoTls};

#[derive(Debug)]
pub struct ServerInfo {
    pub id: i32,
    pub name: String,
    pub host: String,
    pub port: i32,
    pub username: String,
    pub password: String,
}

pub struct DatabaseManager {
    sqlite_conn: Connection,
    server_info: Arc<Mutex<HashMap<i32, ServerInfo>>>,
}

impl DatabaseManager {
    pub fn new(db_path: &str) -> Result<Self> {
        let sqlite_conn = Connection::open(db_path)?;
        let server_info = Arc::new(Mutex::new(HashMap::new()));
        Ok(DatabaseManager { sqlite_conn, server_info })
    }

    pub fn load_server_info(&mut self) -> Result<()> {
        let mut stmt = self.sqlite_conn.prepare("SELECT id, name, host, port, username, password FROM servers")?;
        let server_iter = stmt.query_map([], |row| {
            Ok(ServerInfo {
                id: row.get(0)?,
                name: row.get(1)?,
                host: row.get(2)?,
                port: row.get(3)?,
                username: row.get(4)?,
                password: row.get(5)?,
            })
        })?;

        let mut server_map = self.server_info.lock().unwrap();
        for server in server_iter {
            let server_info = server?;
            server_map.insert(server_info.id, server_info);
        }
        Ok(())
    }

    pub async fn connect_to_postgres(&self, server_id: i32) -> Result<Client, tokio_postgres::Error> {
        let server_map = self.server_info.lock().unwrap();
        if let Some(server) = server_map.get(&server_id) {
            let (client, connection) = tokio_postgres::connect(
                &format!("host={} port={} user={} password={}", server.host, server.port, server.username, server.password),
                NoTls,
            ).await?;

            tokio::spawn(async move {
                if let Err(e) = connection.await {
                    eprintln!("Connection error: {}", e);
                }
            });

            Ok(client)
        } else {
            Err(tokio_postgres::Error::from(std::io::Error::new(std::io::ErrorKind::NotFound, "Server not found")))
        }
    }
}