import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { call } from './client';
import { RustCommand } from './commands';
import {
  CATALOG_EVENT,
  type CatalogDiagnostics,
  type CatalogDriftReport,
  type CatalogEvent,
  type CatalogFolder,
  type CatalogObject,
  type CatalogObjectKind,
  type CatalogObjectRef,
  type CatalogNode,
  type CatalogPage,
  type CatalogPath,
  type CatalogResolution,
  type CatalogSearchHit,
  type CatalogStatus,
  type SchemaComparison,
  type ShapeGroup,
} from './types/catalog.types';

/**
 * Catálogo de metadados no backend (Postgres, Mongo, SQLite). O backend guarda a estrutura
 * inteira; estas funções pedem fatias. O progresso chega por `onCatalogEvent`.
 */

/** Abre o catálogo do database (do disco, se houver) e revalida se preciso */
export function catalogOpen(
  serverId: number,
  database: string,
): Promise<CatalogStatus> {
  return call<CatalogStatus>(RustCommand.CatalogOpen, { serverId, database });
}

export function catalogChildren(
  serverId: number,
  database: string,
  path: CatalogPath,
  options: { filter?: string | null; offset?: number; limit: number },
): Promise<CatalogPage<CatalogNode>> {
  return call<CatalogPage<CatalogNode>>(RustCommand.CatalogChildren, {
    serverId,
    database,
    path,
    filter: options.filter || null,
    offset: options.offset ?? 0,
    limit: options.limit,
  });
}

/**
 * Sem `serverId`/`database`: busca em todos os catálogos abertos (palette).
 * Com `schema`: só as relações desse schema (nome exato), e a busca vazia
 * lista todas — o schema fixado na palette.
 */
export function catalogSearch(
  query: string,
  limit: number,
  scope?: { serverId: number; database: string },
  schema?: string | null,
): Promise<CatalogSearchHit[]> {
  return call<CatalogSearchHit[]>(RustCommand.CatalogSearch, {
    query,
    limit,
    serverId: scope?.serverId ?? null,
    database: scope?.database ?? null,
    schema: schema ?? null,
  });
}

export function catalogResolve(
  serverId: number,
  database: string,
  table: string,
  searchPath: string[],
): Promise<CatalogResolution> {
  return call<CatalogResolution>(RustCommand.CatalogResolve, {
    serverId,
    database,
    table,
    searchPath,
  });
}

/** Com `schema`: relações dele por prefixo; sem: schemas por prefixo */
export function catalogComplete(
  serverId: number,
  database: string,
  schema: string | null,
  prefix: string,
  limit: number,
): Promise<CatalogNode[]> {
  return call<CatalogNode[]>(RustCommand.CatalogComplete, {
    serverId,
    database,
    schema,
    prefix,
    limit,
  });
}

export function catalogDrift(
  serverId: number,
  database: string,
): Promise<CatalogDriftReport> {
  return call<CatalogDriftReport>(RustCommand.CatalogDrift, {
    serverId,
    database,
  });
}

/**
 * `other` comparado com `base`: tabelas que só um tem e, nas comuns, as colunas
 * que diferem. Vai ao banco só pelas colunas das tabelas comuns.
 */
export function catalogCompareSchemas(
  serverId: number,
  database: string,
  base: string,
  other: string,
): Promise<SchemaComparison> {
  return call<SchemaComparison>(RustCommand.CatalogCompareSchemas, {
    serverId,
    database,
    base,
    other,
  });
}

/** Com `schema` recarrega só ele (na hora); sem, revalida o database todo */
export function catalogRefresh(
  serverId: number,
  database: string,
  schema?: string | null,
): Promise<CatalogStatus> {
  return call<CatalogStatus>(RustCommand.CatalogRefresh, {
    serverId,
    database,
    schema: schema ?? null,
  });
}

export function catalogCancel(
  serverId: number,
  database: string,
): Promise<boolean> {
  return call<boolean>(RustCommand.CatalogCancel, { serverId, database });
}

export function catalogRelationSize(
  serverId: number,
  database: string,
  schema: string,
  table: string,
): Promise<number | null> {
  return call<number | null>(RustCommand.CatalogRelationSize, {
    serverId,
    database,
    schema,
    table,
  });
}

/** Grupos por formato (árvore agrupada); vazio quando não há formato repetido */
export function catalogShapes(
  serverId: number,
  database: string,
): Promise<ShapeGroup[]> {
  return call<ShapeGroup[]>(RustCommand.CatalogShapes, { serverId, database });
}

export function catalogDiagnostics(
  serverId: number,
  database: string,
): Promise<CatalogDiagnostics> {
  return call<CatalogDiagnostics>(RustCommand.CatalogDiagnostics, {
    serverId,
    database,
  });
}

/**
 * Uma janela de uma pasta (views, views materializadas, índices, funções) de
 * um schema (`FLAT_SCHEMA` no Mongo/SQLite). Com `table`, só os índices dessa
 * relação — no Mongo, a única forma. Índices e funções vão ao banco só na
 * primeira vez.
 */
export function catalogObjects(
  serverId: number,
  database: string,
  schema: string,
  kind: CatalogObjectKind,
  options: {
    table?: string | null;
    filter?: string | null;
    offset?: number;
    limit: number;
  },
): Promise<CatalogPage<CatalogObject>> {
  return call<CatalogPage<CatalogObject>>(RustCommand.CatalogObjects, {
    serverId,
    database,
    schema,
    kind,
    table: options.table ?? null,
    filter: options.filter || null,
    offset: options.offset ?? 0,
    limit: options.limit,
  });
}

/** As pastas de um schema, com a contagem quando já se sabe (sem ir ao banco) */
export function catalogFolders(
  serverId: number,
  database: string,
  schema: string,
): Promise<CatalogFolder[]> {
  return call<CatalogFolder[]>(RustCommand.CatalogFolders, {
    serverId,
    database,
    schema,
  });
}

/** A definição completa de um objeto (`CREATE VIEW…`, `CREATE INDEX…`…) */
export function catalogObjectDefinition(
  serverId: number,
  database: string,
  target: CatalogObjectRef,
): Promise<string> {
  return call<string>(RustCommand.CatalogObjectDefinition, {
    serverId,
    database,
    target,
  });
}

/** Progresso dos catálogos (evento global do Tauri). Devolve o cancelamento. */
export function onCatalogEvent(
  handler: (event: CatalogEvent) => void,
): Promise<UnlistenFn> {
  return listen<CatalogEvent>(CATALOG_EVENT, event => handler(event.payload));
}
