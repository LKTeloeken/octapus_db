import { call } from './client';
import { RustCommand } from './commands';
import type { StatementResult } from './types/query.types';
import type {
  ColumnInfo,
  DatabaseInfo,
  DatabaseStructure,
  IndexInfo,
  SchemaInfo,
  StructureChange,
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

/** O SQL que a mudança vai rodar, sem aplicar (o diálogo mostra antes) */
export function previewStructureChange(
  serverId: number,
  database: string,
  change: StructureChange,
): Promise<string[]> {
  return call<string[]>(RustCommand.PreviewStructureChange, {
    serverId,
    database,
    change,
  });
}

/** Cria, renomeia ou remove uma tabela ou coluna, numa transação */
export function applyStructureChange(
  serverId: number,
  database: string,
  change: StructureChange,
): Promise<StatementResult> {
  return call<StatementResult>(RustCommand.ApplyStructureChange, {
    serverId,
    database,
    change,
  });
}
