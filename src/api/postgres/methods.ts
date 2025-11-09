import { invoke } from "../utils/invokeHandler";
import { RustMethods } from "../rust-functions";
import type * as Database from "@/shared/models/database.types";

export async function connectToServer(
  serverId: number,
  databaseName?: string
): Promise<boolean> {
  const hasConnected = await invoke<boolean>(RustMethods.CONNECT_TO_SERVER, {
    serverId,
    databaseName,
  });

  return hasConnected;
}

export async function executeQuery(
  serverId: number,
  query: string,
  databaseName?: string
): Promise<{ rows: any[]; fields?: string[] }> {
  const { rows } = await invoke<{ rows: any[] }>(
    RustMethods.RUN_POSTGRE_QUERY,
    {
      serverId,
      query,
      databaseName,
    }
  );
  const fields = rows ? Object.keys(rows?.[0]).sort() : [];

  return { fields, rows };
}

export async function getDatabases(
  serverId: number
): Promise<Database.Database[]> {
  const databases = await invoke<Database.Database[]>(
    RustMethods.GET_POSTGRE_DATABASES,
    {
      serverId,
    }
  );

  return databases;
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
