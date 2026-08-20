import type { QueryClient } from '@tanstack/react-query';
import type { SortSpec } from '@/api/types/browse.types';

/**
 * Hierarchical key factory — invalidation works by prefix:
 * invalidating `tableDataForTable(...)` hits every where/sort variation
 * of that table.
 */
export const queryKeys = {
  servers: ['servers'] as const,

  capabilities: (serverId: number) => ['capabilities', serverId] as const,

  databases: (serverId: number) => ['databases', serverId] as const,

  structure: (serverId: number, database: string) =>
    ['structure', serverId, database] as const,

  /** Prefixo de todas as structures de um servidor */
  structureScope: (serverId: number) => ['structure', serverId] as const,

  columns: (
    serverId: number,
    database: string,
    schema: string,
    table: string,
  ) => ['columns', serverId, database, schema, table] as const,

  /** Prefixo das colunas de um servidor, de um database ou de um schema */
  columnsScope: (serverId: number, database?: string, schema?: string) => {
    if (database === undefined) return ['columns', serverId] as const;
    if (schema === undefined) return ['columns', serverId, database] as const;
    return ['columns', serverId, database, schema] as const;
  },

  indexes: (
    serverId: number,
    database: string,
    schema: string,
    table: string,
  ) => ['indexes', serverId, database, schema, table] as const,

  tableDataForTable: (
    serverId: number,
    database: string,
    schema: string | null,
    table: string,
  ) => ['table-data', serverId, database, schema ?? '', table] as const,

  tableData: (
    serverId: number,
    database: string,
    schema: string | null,
    table: string,
    whereExpr: string,
    sort: SortSpec[],
  ) =>
    [
      'table-data',
      serverId,
      database,
      schema ?? '',
      table,
      { whereExpr, sort },
    ] as const,
};

/** Drops every cached query scoped to a server (key shape: [domain, serverId, ...]) */
export function invalidateServerScope(
  queryClient: QueryClient,
  serverId: number,
) {
  return queryClient.invalidateQueries({
    predicate: query => query.queryKey[1] === serverId,
  });
}

export function removeServerScope(queryClient: QueryClient, serverId: number) {
  queryClient.removeQueries({
    predicate: query => query.queryKey[1] === serverId,
  });
}
