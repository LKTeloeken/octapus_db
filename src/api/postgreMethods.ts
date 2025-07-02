import { invoke } from "./utils/invokeHandler";
import * as IPostgres from "@/shared/models/postgreDb";
import { IServer } from "@/shared/models/server";

export async function connectToPostgreServer(
  serverId: number,
  databaseName?: string
): Promise<boolean> {
  const hasConnected = await invoke<boolean>("connect_to_server", {
    serverId,
    databaseName,
  });

  return hasConnected;
}

// Funções para obter dados do PostgreSQL
export async function getPostgreSchemas(
  serverId: number,
  databaseName?: string
): Promise<IPostgres.IPostgreSchema[]> {
  const schemas = await invoke<IPostgres.IPostgreSchema[]>(
    "get_postgre_schemas",
    { serverId, databaseName }
  );

  console.log("Schemas fetched:", schemas);

  // Convertendo para o formato esperado pela interface IPostgreSchema
  return schemas.map((schema) => ({
    name: schema.name,
    tables: [],
    views: [],
    sequences: [],
  }));
}

export async function getPostgreDatabases(
  serverId: number
): Promise<IPostgres.IPostgreDatabase[]> {
  const databases = await invoke<IPostgres.IPostgreDatabasePrimitive[]>(
    "get_postgre_databases",
    { serverId }
  );

  console.log("Databases fetched:", databases);

  return databases.map((db) => ({
    name: db.datname,
    server_id: serverId,
  }));
}

export async function getPostgreTables(
  serverId: number,
  schemaName: string,
  databaseName?: string
): Promise<IPostgres.IPostgreTable[]> {
  const tables = await invoke<
    { name: string; schema: string; table_type: string }[]
  >("get_postgre_tables", { schemaName, databaseName, serverId });

  // Convertendo para o formato esperado pela interface IPostgreTable
  return tables.map((table) => ({
    name: table.name,
    schema: table.schema,
    table_type: table.table_type,
    triggers: [],
    columns: [],
    indexes: [],
    primary_keys: [],
    foreign_keys: [],
  }));
}

export async function getPostgreColumns(
  server_id: number,
  schemaName: string,
  tableName: string,
  databaseName?: string
): Promise<IPostgres.IPostgreColumn[]> {
  return await invoke<IPostgres.IPostgreColumn[]>("get_postgre_columns", {
    schemaName,
    tableName,
    databaseName,
    server_id,
  });
}
