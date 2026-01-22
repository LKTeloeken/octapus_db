export enum RustMethods {
  // Server methods
  GET_ALL_SERVERS = "get_all_servers",
  GET_SERVER_BY_ID = "get_server_by_id",
  CREATE_SERVER = "create_server",
  UPDATE_SERVER = "update_server",
  DELETE_SERVER = "delete_server",

  // Database methods
  GET_DATABASES = "list_databases",
  GET_SCHEMAS_WITH_TABLES = "list_schemas_with_tables",
  GET_COLUMNS = "list_columns",

  // PostgreSQL methods
  CONNECT_TO_SERVER = "connect",
  RUN_POSTGRE_QUERY = "run_postgre_query",
  GET_POSTGRE_SCHEMAS = "get_postgre_schemas",
  GET_POSTGRE_DATABASES = "get_postgre_databases",
  GET_POSTGRE_TABLES = "get_postgre_tables",
  GET_POSTGRE_COLUMNS = "get_postgre_columns",
  GET_POSTGRE_STRUCTURE = "get_database_structure",
}
