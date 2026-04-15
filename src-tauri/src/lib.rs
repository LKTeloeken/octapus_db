mod adapters;
mod commands;
mod error;
mod models;
mod services;
mod state;
mod storage;

use state::AppState;
use storage::init_storage;
use tauri::{Builder, Manager};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    Builder::default()
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data directory");

            std::fs::create_dir_all(&app_data_dir)
                .expect("Failed to create app data directory");

            let storage_conn =
                init_storage(&app_data_dir.join("app.db")).expect("Failed to initialize storage");

            app.manage(AppState::new(storage_conn));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Servers
            commands::create_server,
            commands::get_all_servers,
            commands::get_server,
            commands::update_server,
            commands::delete_server,
            // Connections
            commands::connect,
            commands::disconnect,
            commands::test_connection,
            commands::get_pool_stats,
            // Queries
            commands::execute_query,
            commands::execute_statement,
            commands::apply_row_edits,
            commands::insert_table_rows,
            commands::delete_table_rows,
            commands::execute_transaction,
            commands::cancel_query,
            // Structure (lazy loading)
            commands::list_databases,
            commands::list_schemas,
            commands::list_tables,
            commands::list_columns,
            commands::list_indexes,
            commands::list_schemas_with_tables,
        ])
        .plugin(tauri_plugin_opener::init())
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
