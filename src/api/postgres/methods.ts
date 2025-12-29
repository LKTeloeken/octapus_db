import { invoke } from "../utils/invokeHandler";
import { RustMethods } from "../rust-functions";
import * as Database from "@/shared/models/database.types";
import type {
  ConnectToServerResponse,
  ExecutePostgreQueryResponse,
  ExecuteQueryResponse,
} from "./response.types";

export async function connectToServer(
  serverId: number,
  databaseName?: string
): Promise<ConnectToServerResponse> {
  const hasConnected = await invoke<ConnectToServerResponse>(
    RustMethods.CONNECT_TO_SERVER,
    {
      serverId,
      databaseName,
    }
  );

  return hasConnected;
}

export async function executeQuery(
  serverId: number,
  query: string,
  databaseName?: string
): Promise<ExecuteQueryResponse> {
  try {
    const { rows, columns, has_more, row_count, total_count } =
      await invoke<ExecutePostgreQueryResponse>(RustMethods.RUN_POSTGRE_QUERY, {
        serverId,
        query,
        databaseName,
        options: { unlimited: true },
      });

    const fields = columns ? columns.map(({ name }) => name) : [];

    return { fields, rows };
  } catch (error) {
    return Promise.reject(error);
  }
}

export async function getDatabases(
  serverId: number
): Promise<Database.Database[]> {
  const databases = await invoke<{ datname: string }[]>(
    RustMethods.GET_POSTGRE_DATABASES,
    {
      serverId,
    }
  );

  return databases.map((db) => ({ name: db.datname, serverId }));
}

export async function getSchemas(
  serverId: number,
  databaseName?: string
): Promise<Database.Schema[]> {
  const schemas = await invoke<Database.Schema[]>(
    RustMethods.GET_POSTGRE_SCHEMAS,
    {
      serverId,
      databaseName,
    }
  );

  return schemas;
}

export async function getTables(
  serverId: number,
  databaseName: string,
  schemaName: string
): Promise<Database.Table[]> {
  const tables = await invoke<Database.Table[]>(
    RustMethods.GET_POSTGRE_TABLES,
    {
      serverId,
      databaseName,
      schemaName,
    }
  );

  return tables;
}

export async function getColumns(
  serverId: number,
  databaseName: string,
  schemaName: string,
  tableName: string
): Promise<Database.Column[]> {
  const columns = await invoke<Database.Column[]>(
    RustMethods.GET_POSTGRE_COLUMNS,
    {
      serverId,
      databaseName,
      schemaName,
      tableName,
    }
  );

  return columns;
}

export async function getPostgreStructure(
  serverId: number,
  databaseName: string
): Promise<Database.DatabaseStructure> {
  const structure = await invoke<Database.DatabaseStructure>(
    RustMethods.GET_POSTGRE_STRUCTURE,
    {
      serverId,
      databaseName,
    }
  );

  return structure;
}
