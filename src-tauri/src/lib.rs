mod db;
mod app_state;
mod commands;
mod models;
mod db_connection_manager;

use std::sync::Mutex;

use tauri::{Builder, Manager};
use db::init_database;
use app_state::AppState;
use commands::servers_commands::{create_server, get_all_servers, get_server_by_id, update_server, delete_server};
use commands::postgre_commands::{run_postgre_query, connect_to_server, get_postgre_databases, get_postgre_schemas, get_postgre_tables, get_postgre_columns, get_postgre_triggers, get_postgre_indexes};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    Builder::default()
        .setup(|app| {
            // Get the app data directory (safe location on macOS)
            let app_data_dir = app.path()
                .app_data_dir()
                .expect("Failed to get app data directory");
            
            // Ensure the directory exists
            std::fs::create_dir_all(&app_data_dir)
                .expect("Failed to create app data directory");
            
            let db_path = app_data_dir.join("app.db");
            let conn = init_database(&db_path)
                .expect("Failed to initialize database");

            // Create app state
            let app_state = AppState {
                conn: Mutex::new(conn),
            };
            
            app.manage(app_state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            create_server,
            get_all_servers,
            get_server_by_id,
            update_server,
            delete_server,

            connect_to_server,
            run_postgre_query,
            get_postgre_databases,
            get_postgre_schemas,
            get_postgre_tables,
            get_postgre_columns,
            get_postgre_triggers,
            get_postgre_indexes
        ])
        .plugin(tauri_plugin_opener::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}