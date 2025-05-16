use tauri::Manager;
use crate::app_state::AppState;
use crate::db::init_database;

fn main() {
    let db_path = "path/to/your/database.db"; // Specify the path to your SQLite database
    let conn = init_database(db_path).expect("Failed to initialize the database");

    let app_state = AppState {
        conn: std::sync::Mutex::new(conn),
    };

    tauri::Builder::default()
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            // Add your command handlers here
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}