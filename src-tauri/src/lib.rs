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
    let builder = Builder::default()
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("Failed to get app data directory");

            std::fs::create_dir_all(&app_data_dir)
                .expect("Failed to create app data directory");

            // Um cofre trancado (senha mestre) ou corrompido é estado normal de
            // boot — o app precisa abrir para a pessoa digitar a senha ou resetar.
            let vault_state =
                storage::vault::init(&app_data_dir).expect("Failed to initialize secrets vault");

            let storage_conn =
                init_storage(app_data_dir.join("app.db")).expect("Failed to initialize storage");

            // Reescreve o arquivo se algum segredo em texto puro foi migrado
            // para o cofre: o SQLite não sobrescreve páginas liberadas sozinho.
            let _ = storage::health::run_pending_vacuum(&storage_conn);

            // Canário: a chave em vault.key ainda abre o que está guardado?
            // Só dá para julgar com o cofre destravado; se houver senha mestre,
            // a checagem acontece no unlock.
            if vault_state == storage::vault::VaultState::Unlocked {
                storage::health::check_or_seed(&storage_conn);
            }

            app.manage(AppState::new(storage_conn));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Vault
            commands::vault_status,
            commands::vault_unlock,
            commands::vault_lock,
            commands::vault_enable_master_password,
            commands::vault_disable_master_password,
            commands::vault_reset,
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
            commands::insert_rows,
            commands::delete_rows,
            commands::execute_transaction,
            commands::cancel_query,
            // Browse (server-side pagination/sort/filter)
            commands::fetch_table_data,
            commands::get_capabilities,
            // Structure (lazy loading)
            commands::list_databases,
            commands::list_schemas,
            commands::list_tables,
            commands::list_columns,
            commands::list_indexes,
            commands::list_schemas_with_tables,
        ])
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init());

    // O updater só existe em desktop — em mobile a dependência nem é compilada.
    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_updater::Builder::new().build());

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}