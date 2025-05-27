use tauri::State;
use crate::{
    app_state::AppState,
    models::PostgreServer,
    db_connection_manager::{execute_query},
};

#[tauri::command]
pub fn get_postgre_databases(state: State<AppState>, server_id: i32) -> Result<Vec<String>, String> {
    println!("Fetching PostgreSQL databases for server ID: {}", server_id);
    
    // Original database query code commented out
    let query: String = "SELECT * FROM pg_database WHERE datistemplate = false".to_string();
    let result = execute_query(&state, server_id, &query).map_err(|e| e.to_string());
    
    // For logging: You can add a log here
    println!("Returning fixed database list: {:?}", result);

    // match result {
    //     Ok(rows) => {
    //         let databases: Vec<String> = rows.iter()
    //             .map(|row| row.get(0).unwrap_or_default())
    //             .collect();
    //         Ok(databases)
    //     },
    //     Err(e) => Err(e),
    // }

    // Fixed vector of strings for testing
    let databases: Vec<String> = vec![
        "postgres".to_string(),
        "template1".to_string(),
        "mydatabase".to_string(),
        "users".to_string(),
        "testdb".to_string(),
    ];
    
    Ok(databases)
}
 