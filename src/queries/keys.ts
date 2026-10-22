import type { QueryClient } from '@tanstack/react-query';
import type { BackupRequest, RestoreRequest } from '@/api/types/backup.types';
import type { SortSpec } from '@/api/types/browse.types';
import type { CatalogObjectRef, CatalogPath } from '@/api/types/catalog.types';
import type { StructureChange } from '@/api/types/structure.types';

/** Identidade de um nó do catálogo dentro da key (`\u0000` não aparece em nomes) */
export function catalogPathKey(path: CatalogPath): string {
  switch (path.kind) {
    case 'schemas':
      return 'schemas';
    case 'schema':
      return `schema:${path.schema}`;
    case 'tables':
      return `tables:${path.schema}`;
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

  serverGroups: ['server-groups'] as const,

  /** Apps desta máquina com conexões para importar */
  importSources: ['import-sources'] as const,

  settings: ['settings'] as const,

  /** Ferramentas do Postgres — prefixo, invalidado ao trocar a pasta */
  pgTools: ['pgTools'] as const,

  pgToolsDetection: ['pgTools', 'detection'] as const,

  pgToolsStatus: (serverId: number, database: string) =>
    ['pgTools', 'status', serverId, database] as const,

  backupPreview: (request: BackupRequest | null) =>
    ['backupPreview', request] as const,

  restoreInspection: (path: string | null) =>
    ['restoreInspection', path] as const,

  restorePreview: (request: RestoreRequest | null, path: string | null) =>
    ['restorePreview', request, path] as const,

  capabilities: (serverId: number) => ['capabilities', serverId] as const,

  dbCapabilities: ['dbCapabilities'] as const,

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

  /** Grupos por formato (árvore agrupada) */
  catalogShapes: (serverId: number, database: string) =>
    ['catalog', serverId, database, 'shapes'] as const,

  /** Pastas de um schema (quais e quantos itens, quando já se sabe) */
  catalogFolders: (serverId: number, database: string, schema: string) =>
    ['catalog', serverId, database, 'folders', schema] as const,

  /** Uma janela de uma pasta; `table` = os índices de uma relação só */
  catalogObjects: (
    serverId: number,
    database: string,
    schema: string,
    kind: string,
    table: string | null,
    filter: string,
    limit: number,
  ) =>
    [
      'catalog',
      serverId,
      database,
      'objects',
      schema,
      kind,
      table,
      { filter, limit },
    ] as const,

  /** A definição de um objeto (a aba de definição) */
  catalogDefinition: (
    serverId: number,
    database: string,
    target: CatalogObjectRef,
  ) =>
    [
      'catalog',
      serverId,
      database,
      'definition',
      target.kind,
      target.schema,
      target.table ?? null,
      target.name,
      target.signature ?? null,
    ] as const,

  /** Tamanho exato de uma relação (sob demanda: só da tabela aberta) */
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

  /**
   * Comparação entre dois schemas. Fora do prefixo `catalog`: um evento de
   * sincronização não refaz a consulta (que vai ao banco).
   */
  schemaComparisonScope: (serverId: number, database: string) =>
    ['schema-comparison', serverId, database] as const,

  schemaComparison: (
    serverId: number,
    database: string,
    base: string,
    other: string,
  ) => ['schema-comparison', serverId, database, base, other] as const,

  /** SQL de uma mudança de estrutura, antes de aplicar (a key é a mudança) */
  structurePreview: (
    serverId: number,
    database: string,
    change: StructureChange | null,
  ) => ['structure-preview', serverId, database, change] as const,

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

/**
 * Tudo em cache de um database (key `[domínio, serverId, database, ...]`):
 * depois de uma restauração, colunas, dados e catálogo podem ter mudado.
 */
export function invalidateDatabaseScope(
  queryClient: QueryClient,
  serverId: number,
  database: string,
) {
  return queryClient.invalidateQueries({
    predicate: query =>
      query.queryKey[1] === serverId && query.queryKey[2] === database,
  });
}

export function removeServerScope(queryClient: QueryClient, serverId: number) {
  queryClient.removeQueries({
    predicate: query => query.queryKey[1] === serverId,
  });
}
