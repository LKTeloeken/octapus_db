// filepath: /Users/lucasteloeken/Documents/projects/octapus_db/src-tauri/src/commands/connection_commands.rs
use rusqlite::{Connection, Result};
use std::sync::{Arc, Mutex};
use tokio_postgres::{Client, NoTls};
use crate::models::ConnectionDetails;

pub struct DatabaseConnection {
    pub sqlite_conn: Connection,
    pub pg_client: Client,
}

impl DatabaseConnection {
    pub fn new(sqlite_path: &str, connection_details: &ConnectionDetails) -> Result<Self> {
        let sqlite_conn = Connection::open(sqlite_path)?;

        let (pg_client, connection) = tokio_postgres::connect(
            &format!(
                "host={} port={} user={} password={} dbname={}",
                connection_details.host,
                connection_details.port,
                connection_details.username,
                connection_details.password,
                connection_details.database_name
            ),
            NoTls,
        )?;

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("Connection error: {}", e);
            }
        });

        Ok(DatabaseConnection { sqlite_conn, pg_client })
    }

    pub fn execute_query(&self, query: &str) -> Result<Vec<tokio_postgres::Row>> {
        let rows = self.pg_client.query(query, &[])?;
        Ok(rows)
    }
}

pub fn get_connection_details(sqlite_conn: &Connection) -> Result<ConnectionDetails> {
    let mut stmt = sqlite_conn.prepare("SELECT host, port, username, password, database_name FROM servers WHERE id = ?")?;
    let connection_details_iter = stmt.query_map(&[1], |row| {
        Ok(ConnectionDetails {
            host: row.get(0)?,
            port: row.get(1)?,
            username: row.get(2)?,
            password: row.get(3)?,
            database_name: row.get(4)?,
        })
    })?;

    for connection_details in connection_details_iter {
        return connection_details;
    }

    Err(rusqlite::Error::QueryReturnedNoRows)
}