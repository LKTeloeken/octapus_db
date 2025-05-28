import { invoke } from "@tauri-apps/api/core";
import * as IPostgres from "@/models/postgreDb";

// Funções para gerenciamento de servidores PostgreSQL
export async function createServer(
  server: IPostgres.IPostgreServerPrimitive
): Promise<IPostgres.IPostgreServer> {
  return await invoke<IPostgres.IPostgreServer>("create_server", server);
}

export async function getAllServers(): Promise<IPostgres.IPostgreServer[]> {
  return await invoke<IPostgres.IPostgreServer[]>("get_all_servers");
}

export async function getServerById(
  id: number
): Promise<IPostgres.IPostgreServer> {
  return await invoke<IPostgres.IPostgreServer>("get_server_by_id", { id });
}

export async function updateServer(
  id: number,
  name: string,
  host: string,
  port: number,
  username: string,
  password: string
): Promise<IPostgres.IPostgreServer> {
  return await invoke<IPostgres.IPostgreServer>("update_server", {
    id,
    name,
    host,
    port,
    username,
    password,
  });
}

export async function deleteServer(id: number): Promise<void> {
  return await invoke<void>("delete_server", { id });
}

// Funções para obter dados do PostgreSQL
export async function getPostgreSchemas(): Promise<IPostgres.IPostgreSchema[]> {
  const schemas = await invoke<{ name: string }[]>("get_postgre_schemas");

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
  const databases = await invoke<IPostgres.IPostgreDatabase[]>(
    "get_postgre_databases",
    { serverId }
  );

  const schemas = await invoke<IPostgres.IPostgreSchema[]>(
    "get_postgre_schemas",
    { serverId, databaseName: "teste" }
  );

  console.log("Schemas:", schemas);

  return databases;
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
