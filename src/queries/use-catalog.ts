import {
  keepPreviousData,
  queryOptions,
  useMutation,
  useQuery,
  useQueryClient,
} from '@tanstack/react-query';
import {
  catalogChildren,
  catalogDiagnostics,
  catalogOpen,
  catalogRefresh,
  catalogRelationSize,
  catalogSearch,
  catalogShapes,
} from '@/api/catalog';
import type { CatalogPath } from '@/api/types/catalog.types';
import type { DatabaseType } from '@/api/types/server.types';
import { queryKeys } from './keys';

/** Janela padrão de filhos por nó; a árvore pede mais com "carregar mais" */
export const CATALOG_PAGE_SIZE = 500;

/** Quantos resultados a palette pede ao backend */
export const CATALOG_SEARCH_LIMIT = 50;

/**
 * Bancos com catálogo no backend. O Redis segue com a estrutura inteira
 * (`useStructure`): as chaves não têm estrutura fixa.
 */
export function hasCatalog(dbType: DatabaseType | undefined): boolean {
  return dbType === 'postgres' || dbType === 'mongodb' || dbType === 'sqlite';
}

/** Catálogo sem nível de schema: as relações ficam em `FLAT_SCHEMA` */
export function isFlatCatalog(dbType: DatabaseType | undefined): boolean {
  return dbType === 'mongodb' || dbType === 'sqlite';
}

/**
 * O catálogo vive no backend e avisa quando muda (`useCatalogEvents`): nada
 * aqui fica velho sozinho, e nada vai para o IndexedDB (o backend já guarda em
 * disco, cifrado).
 */
const fromEvents = { staleTime: Infinity, gcTime: 10 * 60 * 1000 } as const;

/** Abre o catálogo do database e acompanha o estado (sincronizando, erro…) */
export function catalogStatusQuery(serverId: number, database: string) {
  return queryOptions({
    queryKey: queryKeys.catalogStatus(serverId, database),
    queryFn: () => catalogOpen(serverId, database),
    ...fromEvents,
  });
}

export function useCatalogStatus(
  serverId: number | null | undefined,
  database: string | null | undefined,
  options?: { enabled?: boolean },
) {
  return useQuery({
    ...catalogStatusQuery(serverId ?? -1, database ?? ''),
    enabled: serverId != null && !!database && (options?.enabled ?? true),
  });
}

/**
 * Uma janela dos filhos de um nó. Um schema que a sincronização ainda não
 * alcançou é carregado na hora pelo backend (faixa de prioridade).
 */
export function catalogChildrenQuery(
  serverId: number,
  database: string,
  path: CatalogPath,
  filter = '',
  limit = CATALOG_PAGE_SIZE,
) {
  return queryOptions({
    queryKey: queryKeys.catalogChildren(
      serverId,
      database,
      path,
      filter,
      limit,
    ),
    queryFn: () => catalogChildren(serverId, database, path, { filter, limit }),
    // Filtro e "carregar mais" trocam a key: a janela anterior fica na tela
    // até a nova chegar, sem piscar o nó
    placeholderData: keepPreviousData,
    ...fromEvents,
  });
}

/** Grupos por formato de um database (a árvore agrupada) */
export function catalogShapesQuery(serverId: number, database: string) {
  return queryOptions({
    queryKey: queryKeys.catalogShapes(serverId, database),
    queryFn: () => catalogShapes(serverId, database),
    placeholderData: keepPreviousData,
    ...fromEvents,
  });
}

/**
 * Diagnóstico sob demanda ("Copiar diagnóstico"): sempre do momento, sem
 * cache — é o que o usuário vai colar numa issue.
 */
export function useCatalogDiagnostics() {
  return useMutation({
    mutationFn: (target: { serverId: number; database: string }) =>
      catalogDiagnostics(target.serverId, target.database),
  });
}

export function useCatalogSearch(
  query: string,
  options?: { enabled?: boolean; schema?: string | null },
) {
  const trimmed = query.trim();
  const schema = options?.schema ?? null;
  return useQuery({
    queryKey: queryKeys.catalogSearch(trimmed, CATALOG_SEARCH_LIMIT, schema),
    queryFn: () =>
      catalogSearch(trimmed, CATALOG_SEARCH_LIMIT, undefined, schema),
    // Com schema fixado, a busca vazia lista as relações dele
    enabled:
      (trimmed.length > 0 || schema !== null) && (options?.enabled ?? true),
    placeholderData: keepPreviousData,
    ...fromEvents,
  });
}

/**
 * Tamanho exato de uma relação. Só para a tabela aberta: em massa, o
 * `pg_total_relation_size` lê o disco e incha a memória do servidor, e o
 * `$collStats` do Mongo é um aggregate por coleção (perf/catalog/BASELINE.md,
 * H1, H7 e H8).
 */
export function useCatalogRelationSize(
  target: {
    serverId: number;
    database: string;
    schema: string;
    table: string;
  } | null,
) {
  return useQuery({
    queryKey: queryKeys.catalogRelationSize(
      target?.serverId ?? -1,
      target?.database ?? '',
      target?.schema ?? '',
      target?.table ?? '',
    ),
    queryFn: () =>
      catalogRelationSize(
        target!.serverId,
        target!.database,
        target!.schema,
        target!.table,
      ),
    enabled: target !== null,
    staleTime: 60 * 1000,
  });
}

/**
 * Refresh manual: um schema (recarrega na hora) ou o database inteiro
 * (revalida em segundo plano; o resultado chega pelos eventos).
 */
export function useCatalogRefresh() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (target: {
      serverId: number;
      database: string;
      schema?: string | null;
    }) => catalogRefresh(target.serverId, target.database, target.schema),
    onSuccess: (status, target) => {
      queryClient.setQueryData(
        queryKeys.catalogStatus(target.serverId, target.database),
        status,
      );
      if (target.schema) {
        void queryClient.invalidateQueries({
          queryKey: queryKeys.catalogScope(target.serverId, target.database),
        });
      }
    },
  });
}
