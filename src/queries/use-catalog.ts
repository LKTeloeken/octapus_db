import {
  keepPreviousData,
  queryOptions,
  useMutation,
  useQuery,
  useQueryClient,
} from '@tanstack/react-query';
import {
  catalogChildren,
  catalogComplete,
  catalogCompareSchemas,
  catalogDiagnostics,
  catalogFolders,
  catalogObjectDefinition,
  catalogObjects,
  catalogOpen,
  catalogRefresh,
  catalogRelationSize,
  catalogSearch,
  catalogShapes,
} from '@/api/catalog';
import type {
  CatalogObjectKind,
  CatalogObjectRef,
  CatalogPath,
} from '@/api/types/catalog.types';
import type { AdapterCapabilities } from '@/api/types/capabilities.types';
import { queryKeys } from './keys';

/** Janela padrão de filhos por nó; a árvore pede mais com "carregar mais" */
export const CATALOG_PAGE_SIZE = 500;

/** Quantos resultados a palette pede ao backend */
export const CATALOG_SEARCH_LIMIT = 50;

/**
 * Banco com catálogo no backend (`caps` de `useDbCapabilities`). O Redis
 * segue com a estrutura inteira (`useStructure`): as chaves não têm
 * estrutura fixa.
 */
export function hasCatalog(caps: AdapterCapabilities | undefined): boolean {
  return !!caps?.hasCatalog;
}

/** Catálogo sem nível de schema: as relações ficam em `FLAT_SCHEMA` */
export function isFlatCatalog(caps: AdapterCapabilities | undefined): boolean {
  return !!caps?.hasCatalog && !caps.hasSchemas;
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

/**
 * As pastas de um schema aberto (as do `structureFolders` do banco) e quantos
 * itens cada uma tem quando já se sabe: views pelo catálogo; índices e funções
 * só depois de abertas (`count: null` até lá).
 */
export function catalogFoldersQuery(
  serverId: number,
  database: string,
  schema: string,
) {
  return queryOptions({
    queryKey: queryKeys.catalogFolders(serverId, database, schema),
    queryFn: () => catalogFolders(serverId, database, schema),
    placeholderData: keepPreviousData,
    ...fromEvents,
  });
}

/**
 * Uma janela de uma pasta aberta. Índices e funções vão ao banco na primeira
 * vez (um schema por consulta, ou uma coleção no Mongo) e ficam em memória no
 * backend até a próxima sincronização.
 */
export function catalogObjectsQuery(
  serverId: number,
  database: string,
  schema: string,
  kind: CatalogObjectKind,
  table: string | null,
  filter = '',
  limit = CATALOG_PAGE_SIZE,
) {
  return queryOptions({
    queryKey: queryKeys.catalogObjects(
      serverId,
      database,
      schema,
      kind,
      table,
      filter,
      limit,
    ),
    queryFn: () =>
      catalogObjects(serverId, database, schema, kind, {
        table,
        filter,
        limit,
      }),
    placeholderData: keepPreviousData,
    ...fromEvents,
  });
}

/**
 * Definição de um objeto, sob demanda e sempre do momento ("Ver definição"):
 * a árvore abre o texto numa aba de query nova.
 */
export function catalogDefinitionQuery(
  serverId: number,
  database: string,
  target: CatalogObjectRef,
) {
  return queryOptions({
    queryKey: queryKeys.catalogDefinition(serverId, database, target),
    queryFn: () => catalogObjectDefinition(serverId, database, target),
    // Muda só com a estrutura: os eventos do catálogo invalidam a key
    staleTime: 5 * 60 * 1000,
  });
}

/** Quantos nomes um `catalog_complete` devolve (a key não leva o limite) */
export const CATALOG_COMPLETE_LIMIT = 50;

/**
 * Completar por prefixo, numa fatia: sem `schema`, os schemas; com ele, as
 * relações dele (no catálogo sem schema, `FLAT_SCHEMA`). Para autocomplete —
 * nunca a estrutura inteira.
 */
export function catalogCompleteQuery(
  serverId: number,
  database: string,
  schema: string | null,
  prefix: string,
) {
  return queryOptions({
    queryKey: queryKeys.catalogComplete(serverId, database, schema, prefix),
    queryFn: () =>
      catalogComplete(
        serverId,
        database,
        schema,
        prefix,
        CATALOG_COMPLETE_LIMIT,
      ),
    staleTime: 30 * 1000,
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

/**
 * `other` comparado com `base` (sem par, não vai ao backend). Refaz a cada
 * abertura do diálogo; fora do prefixo do catálogo e sem retry.
 */
export function useSchemaComparison(
  target: {
    serverId: number;
    database: string;
    base: string;
    other: string;
  } | null,
) {
  return useQuery({
    queryKey: queryKeys.schemaComparison(
      target?.serverId ?? -1,
      target?.database ?? '',
      target?.base ?? '',
      target?.other ?? '',
    ),
    queryFn: () =>
      catalogCompareSchemas(
        target!.serverId,
        target!.database,
        target!.base,
        target!.other,
      ),
    enabled: target !== null,
    staleTime: 0,
    gcTime: 60_000,
    retry: false,
  });
}
