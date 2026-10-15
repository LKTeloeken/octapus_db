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

  // Edição da estrutura (tabelas e colunas) pela árvore
  PreviewStructureChange = 'preview_structure_change',
  ApplyStructureChange = 'apply_structure_change',

  // Catálogo de metadados (árvore, busca, autocomplete em escala)
  CatalogOpen = 'catalog_open',
  CatalogStatus = 'catalog_status',
  CatalogChildren = 'catalog_children',
  CatalogSearch = 'catalog_search',
  CatalogResolve = 'catalog_resolve',
  CatalogComplete = 'catalog_complete',
  CatalogDrift = 'catalog_drift',
  CatalogCompareSchemas = 'catalog_compare_schemas',
  CatalogRefresh = 'catalog_refresh',
  CatalogCancel = 'catalog_cancel',
  CatalogRelationSize = 'catalog_relation_size',
  CatalogShapes = 'catalog_shapes',
  CatalogDiagnostics = 'catalog_diagnostics',

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

  // Exportação de resultados
  WriteExportFile = 'write_export_file',

  // Sessão do workspace (abas abertas)
  LoadSession = 'load_session',
  SaveSession = 'save_session',

  // Preferências do app
  GetSettings = 'get_settings',
  UpdateSettings = 'update_settings',

  // Backup e restauração (ferramentas nativas do Postgres)
  PgToolsDetect = 'pg_tools_detect',
  PgToolsStatus = 'pg_tools_status',
  BackupPreview = 'backup_preview',
  BackupStart = 'backup_start',
  BackupCancel = 'backup_cancel',
  RestoreInspect = 'restore_inspect',
  RestorePreview = 'restore_preview',
  RestoreStart = 'restore_start',

  // Auto-update pelo canal beta (o estável usa o `check` do plugin)
  CheckBetaUpdate = 'check_beta_update',
}
