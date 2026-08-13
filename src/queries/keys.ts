import type { QueryClient } from '@tanstack/react-query';
import type { ColumnFilter, SortSpec } from '@/api/types/browse.types';

/**
 * Hierarchical key factory — invalidation works by prefix:
 * invalidating `tableDataForTable(...)` hits every filter/sort variation
 * of that table.
 */
export const queryKeys = {
  servers: ['servers'] as const,

  capabilities: (serverId: number) => ['capabilities', serverId] as const,

  databases: (serverId: number) => ['databases', serverId] as const,

  structure: (serverId: number, database: string) =>
    ['structure', serverId, database] as const,

  columns: (
    serverId: number,
    database: string,
    schema: string,
    table: string,
  ) => ['columns', serverId, database, schema, table] as const,

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
    filters: ColumnFilter[],
    sort: SortSpec[],
  ) =>
    [
      'table-data',
      serverId,
      database,
      schema ?? '',
      table,
      { filters, sort },
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
