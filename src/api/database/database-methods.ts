import { RustMethods } from "../rust-functions";
import { invoke } from "../utils/invokeHandler";
import type * as Database from "@/shared/models/database.types";
import type {
  GetDatabasesResponse,
  GetSchemasWithTablesResponse,
  GetColumnsResponse,
  ExecuteQueryResponse,
} from "./database-responses.types";

export async function getDatabases(
  serverId: number,
): Promise<Database.Database[]> {
  const databases = await invoke<GetDatabasesResponse[]>(
    RustMethods.GET_DATABASES,
    {
      serverId,
    },
  );

  return databases.map((db) => ({ name: db.name, serverId }));
}

export async function getSchemasWithTables(
  serverId: number,
  database: string,
): Promise<Database.SchemaStructure[]> {
  const { schemas } = await invoke<GetSchemasWithTablesResponse>(
    RustMethods.GET_SCHEMAS_WITH_TABLES,
    {
      serverId,
      database,
    },
  );

  return schemas;
}

export async function getColumns(
  serverId: number,
  database: string,
  schema: string,
  table: string,
): Promise<Database.Column[]> {
  const columns = await invoke<GetColumnsResponse>(RustMethods.GET_COLUMNS, {
    serverId,
    database,
    schema,
    table,
  });

  console.log("cols", columns);

  return columns;
}

export async function executeQuery(
  serverId: number,
  database: string,
  query: string,
): Promise<ExecuteQueryResponse> {
  const result = await invoke<ExecuteQueryResponse>(RustMethods.EXECUTE_QUERY, {
    serverId,
    database,
    query,
  });

  console.log("result", result);

  return result;
}
