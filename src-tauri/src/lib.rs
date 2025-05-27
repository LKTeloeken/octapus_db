mod db;
mod app_state;
mod commands;
mod models;
mod db_connection_manager;

use std::sync::Mutex;

use tauri::Builder;
use db::init_database;
use app_state::AppState;
use commands::servers_commands::{create_server, get_all_servers, get_server_by_id, update_server, delete_server};
use commands::postgre_commands::get_postgre_databases;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let conn = init_database("app.db").expect("Failed to initialize database");

    // Cria o estado da aplicação
    let app_state = AppState {
        conn: Mutex::new(conn),
    };

    Builder::default()
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            create_server,
            get_all_servers,
            get_server_by_id,
            update_server,
            delete_server,

            get_postgre_databases
        ])
        .setup(|app| {
            // `anyhow::anyhow!` agora funciona porque adicionamos a dependência
            // db::init(app).map_err(|e| anyhow::anyhow!(e))?;
            Ok(())
        })
        .plugin(tauri_plugin_opener::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}