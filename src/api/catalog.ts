import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { call } from './client';
import { RustCommand } from './commands';
import {
  CATALOG_EVENT,
  type CatalogDiagnostics,
  type CatalogDriftReport,
  type CatalogEvent,
  type CatalogNode,
  type CatalogPage,
  type CatalogPath,
  type CatalogResolution,
  type CatalogSearchHit,
  type CatalogStatus,
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

/** Sem `serverId`/`database`: busca em todos os catálogos abertos (palette) */
export function catalogSearch(
  query: string,
  limit: number,
  scope?: { serverId: number; database: string },
): Promise<CatalogSearchHit[]> {
  return call<CatalogSearchHit[]>(RustCommand.CatalogSearch, {
    query,
    limit,
    serverId: scope?.serverId ?? null,
    database: scope?.database ?? null,
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

/** Progresso dos catálogos (evento global do Tauri). Devolve o cancelamento. */
export function onCatalogEvent(
  handler: (event: CatalogEvent) => void,
): Promise<UnlistenFn> {
  return listen<CatalogEvent>(CATALOG_EVENT, event => handler(event.payload));
}
