import { invoke } from "./utils/invokeHandler";
import * as IPostgres from "@/models/postgreDb";
import { IServer } from "@/models/server";

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
  server: IServer
): Promise<IPostgres.IPostgreDatabase[]> {
  const databases = await invoke<IPostgres.IPostgreDatabasePrimitive[]>(
    "get_postgre_databases",
    { serverId: server.id }
  );

  console.log("Databases fetched:", databases);

  return databases.map((db) => ({
    name: db.datname,
    server_id: server.id,
    isConnected: server.isConnected,
    schemas: [],
  }));
}

export async function getPostgreTables(
  schema: string
): Promise<IPostgres.IPostgreTable[]> {
  const tables = await invoke<
    { name: string; schema: string; table_type: string }[]
  >("get_postgre_tables", { schema });

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
  schema: string,
  table: string
): Promise<IPostgres.IPostgreColumn[]> {
  return await invoke<IPostgres.IPostgreColumn[]>("get_postgre_columns", {
    schema,
    table,
  });
}
