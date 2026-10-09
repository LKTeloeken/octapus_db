import type { QueryClient } from '@tanstack/react-query';
import type { SortSpec } from '@/api/types/browse.types';
import type { CatalogPath } from '@/api/types/catalog.types';

/** Identidade de um nó do catálogo dentro da key (`\u0000` não aparece em nomes) */
export function catalogPathKey(path: CatalogPath): string {
  switch (path.kind) {
    case 'schemas':
      return 'schemas';
    case 'schema':
      return `schema:${path.schema}`;
    case 'partitions':
      return `partitions:${path.schema}\u0000${path.table}`;
    case 'shape':
      return `shape:${path.key}`;
  }
}

/**
 * Hierarchical key factory — invalidation works by prefix:
 * invalidating `tableDataForTable(...)` hits every where/sort variation
 * of that table.
 */
export const queryKeys = {
  servers: ['servers'] as const,

  settings: ['settings'] as const,

  capabilities: (serverId: number) => ['capabilities', serverId] as const,

  databases: (serverId: number) => ['databases', serverId] as const,

  structure: (serverId: number, database: string) =>
    ['structure', serverId, database] as const,

  /** Prefixo de todas as structures de um servidor */
  structureScope: (serverId: number) => ['structure', serverId] as const,

  /** Tudo do catálogo de um database (backend) — prefixo para os eventos invalidarem */
  catalogScope: (serverId: number, database: string) =>
    ['catalog', serverId, database] as const,

  /** Estado do catálogo; o queryFn é o `catalog_open` (abre e revalida se preciso) */
  catalogStatus: (serverId: number, database: string) =>
    ['catalog', serverId, database, 'status'] as const,

  /** Prefixo dos filhos de um nó: invalidar um schema não toca os outros */
  catalogChildrenOf: (serverId: number, database: string, path: CatalogPath) =>
    ['catalog', serverId, database, 'children', catalogPathKey(path)] as const,

  catalogChildren: (
    serverId: number,
    database: string,
    path: CatalogPath,
    filter: string,
    limit: number,
  ) =>
    [
      'catalog',
      serverId,
      database,
      'children',
      catalogPathKey(path),
      { filter, limit },
    ] as const,

  /** Tamanho exato de uma relação (sob demanda: só da tabela aberta) */
  /** Grupos por formato (árvore agrupada) */
  catalogShapes: (serverId: number, database: string) =>
    ['catalog', serverId, database, 'shapes'] as const,

  catalogRelationSize: (
    serverId: number,
    database: string,
    schema: string,
    table: string,
  ) => ['catalog', serverId, database, 'size', schema, table] as const,

  /** Nome sem schema → relação, seguindo o search_path (autocomplete) */
  catalogResolve: (
    serverId: number,
    database: string,
    table: string,
    searchPath: string[],
  ) => ['catalog', serverId, database, 'resolve', table, searchPath] as const,

  /** Completar schemas (schema `null`) ou relações de um schema por prefixo */
  catalogComplete: (
    serverId: number,
    database: string,
    schema: string | null,
    prefix: string,
  ) => ['catalog', serverId, database, 'complete', schema, prefix] as const,

  /** Busca da palette em todos os catálogos abertos */
  catalogSearchScope: ['catalog-search'] as const,

  catalogSearch: (query: string, limit: number, schema: string | null = null) =>
    ['catalog-search', query, limit, schema] as const,

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
