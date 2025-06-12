import { invoke } from "./utils/invokeHandler";
import * as IPostgres from "@/models/postgreDb";

// Funções para obter dados do PostgreSQL
export async function getPostgreSchemas(
  serverId: number,
  databaseName?: string
): Promise<IPostgres.IPostgreSchema[]> {
  const schemas = await invoke<IPostgres.IPostgreSchema[]>(
    "get_postgre_schemas",
    { serverId, databaseName }
  );

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
