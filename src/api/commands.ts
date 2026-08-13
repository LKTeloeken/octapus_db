/** Tauri commands exposed by the Rust backend (src-tauri/src/lib.rs) */
export enum RustCommand {
  // Servers (SQLite local CRUD)
  GetAllServers = 'get_all_servers',
  GetServer = 'get_server',
  CreateServer = 'create_server',
  UpdateServer = 'update_server',
  DeleteServer = 'delete_server',

  // Connection
  Connect = 'connect',
  TestConnection = 'test_connection',
  Disconnect = 'disconnect',
  GetPoolStats = 'get_pool_stats',
  GetCapabilities = 'get_capabilities',

  // Structure (lazy loading)
  ListDatabases = 'list_databases',
  ListSchemas = 'list_schemas',
  ListTables = 'list_tables',
  ListColumns = 'list_columns',
  ListIndexes = 'list_indexes',
  ListSchemasWithTables = 'list_schemas_with_tables',

  // Free query editor
  ExecuteQuery = 'execute_query',
  ExecuteStatement = 'execute_statement',
  ExecuteTransaction = 'execute_transaction',
  ApplyRowEdits = 'apply_row_edits',
  InsertRows = 'insert_rows',
  DeleteRows = 'delete_rows',
  CancelQuery = 'cancel_query',

  // Browse (server-side pagination/sort/filter)
  FetchTableData = 'fetch_table_data',
}
