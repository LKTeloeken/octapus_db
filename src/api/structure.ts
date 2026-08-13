import { call } from './client';
import { RustCommand } from './commands';
import type {
  ColumnInfo,
  DatabaseInfo,
  DatabaseStructure,
  IndexInfo,
  SchemaInfo,
  TableInfo,
} from './types/structure.types';

export function listDatabases(serverId: number): Promise<DatabaseInfo[]> {
  return call<DatabaseInfo[]>(RustCommand.ListDatabases, { serverId });
}

export function listSchemas(
  serverId: number,
  database: string,
): Promise<SchemaInfo[]> {
  return call<SchemaInfo[]>(RustCommand.ListSchemas, { serverId, database });
}

export function listTables(
  serverId: number,
  database: string,
  schema: string,
): Promise<TableInfo[]> {
  return call<TableInfo[]>(RustCommand.ListTables, {
    serverId,
    database,
    schema,
  });
}

export function listColumns(
  serverId: number,
  database: string,
  schema: string,
  table: string,
): Promise<ColumnInfo[]> {
  return call<ColumnInfo[]>(RustCommand.ListColumns, {
    serverId,
    database,
    schema,
    table,
  });
}

export function listIndexes(
  serverId: number,
  database: string,
  schema: string,
  table: string,
): Promise<IndexInfo[]> {
  return call<IndexInfo[]>(RustCommand.ListIndexes, {
    serverId,
    database,
    schema,
    table,
  });
}

/** Schemas + tables in one call — ideal for the initial sidebar tree */
export function listSchemasWithTables(
  serverId: number,
  database: string,
): Promise<DatabaseStructure> {
  return call<DatabaseStructure>(RustCommand.ListSchemasWithTables, {
    serverId,
    database,
  });
}
