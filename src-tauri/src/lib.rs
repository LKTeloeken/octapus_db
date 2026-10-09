mod adapters;
mod catalog;
mod commands;
mod error;
mod models;
mod services;
mod state;
mod storage;
mod window_chrome;

#[cfg(test)]
mod perf;

use std::sync::Arc;
use std::time::Duration;

use models::CATALOG_EVENT;
use state::AppState;
use storage::init_storage;
use tauri::{Builder, Emitter, Manager, WindowEvent};

const MAINTENANCE_EVERY: Duration = Duration::from_secs(60);
/// Pools sem uso há mais que isto fecham (navegar por dezenas de databases de
/// tenant não pode acumular conexões no servidor do cliente)
const ADAPTER_IDLE_TIMEOUT: Duration = Duration::from_secs(10 * 60);

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

            storage::vault::init(&app_data_dir).expect("Failed to initialize secrets vault");

            let storage_conn =
                init_storage(app_data_dir.join("app.db")).expect("Failed to initialize storage");

            let state = AppState::new(storage_conn);
            let events = app.handle().clone();
            state.catalog.init(
                app_data_dir.join("catalog"),
                Arc::new(move |event| {
                    let _ = events.emit(CATALOG_EVENT, event);
                }),
            );
            app.manage(state);

            // Manutenção: catálogos vencidos saem do disco na partida; a cada
            // minuto, conexões ociosas fecham e o teto de memória é respeitado.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let state = handle.state::<AppState>();
                let _ = state.catalog.purge_expired();
                let mut tick = tokio::time::interval(MAINTENANCE_EVERY);
                tick.tick().await;
                loop {
                    tick.tick().await;
                    state.catalog.maintenance().await;
                    state.connections.evict_idle(ADAPTER_IDLE_TIMEOUT);
                }
            });

            if let Some(window) = app.get_webview_window("main") {
                window_chrome::hide_traffic_lights(&window.as_ref().window());
            }

            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::Resized(_) = event {
                window_chrome::hide_traffic_lights(window);
            }
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
            commands::insert_rows,
            commands::delete_rows,
            commands::execute_transaction,
            commands::cancel_query,
            // Browse (server-side pagination/sort/filter)
            commands::fetch_table_data,
            commands::get_capabilities,
            // Exportação de resultados
            commands::write_export_file,
            // Structure (lazy loading)
            commands::list_databases,
            commands::list_schemas,
            commands::list_tables,
            commands::list_columns,
            commands::list_indexes,
            commands::list_schemas_with_tables,
            // Catálogo de metadados (árvore, busca, autocomplete em escala)
            commands::catalog_open,
            commands::catalog_status,
            commands::catalog_children,
            commands::catalog_search,
            commands::catalog_resolve,
            commands::catalog_complete,
            commands::catalog_drift,
            commands::catalog_refresh,
            commands::catalog_cancel,
            commands::catalog_relation_size,
            commands::catalog_shapes,
            commands::catalog_diagnostics,
            // Sessão do workspace (abas abertas)
            commands::load_session,
            commands::save_session,
        ])
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_process::init());

    // O updater só existe em desktop — em mobile a dependência nem é compilada.
    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_updater::Builder::new().build());

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}