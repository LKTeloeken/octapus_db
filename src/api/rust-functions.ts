export enum RustMethods {
  // Server methods
  GET_ALL_SERVERS = "get_all_servers",
  GET_SERVER_BY_ID = "get_server_by_id",
  CREATE_SERVER = "create_server",
  UPDATE_SERVER = "update_server",
  DELETE_SERVER = "delete_server",

  // PostgreSQL methods
  CONNECT_TO_SERVER = "connect_to_server",
  RUN_POSTGRE_QUERY = "run_postgre_query",
  GET_POSTGRE_SCHEMAS = "get_postgre_schemas",
  GET_POSTGRE_DATABASES = "get_postgre_databases",
  GET_POSTGRE_TABLES = "get_postgre_tables",
  GET_POSTGRE_COLUMNS = "get_postgre_columns",
}
