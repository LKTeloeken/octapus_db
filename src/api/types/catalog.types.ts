/**
 * Contrato do catálogo de metadados (espelho de `src-tauri/src/models/catalog.rs`
 * e dos tipos de saída de `src-tauri/src/catalog/model.rs`). O backend guarda a
 * estrutura inteira; o front pede fatias: filhos de um nó, busca, resolução.
 */

/** Nome do evento global do Tauri com o progresso dos catálogos */
export const CATALOG_EVENT = 'catalog-event';

/**
 * Bancos sem nível de schema (Mongo, SQLite) guardam as relações num schema só,
 * sem nome: pede-se `{ kind: 'schema', schema: FLAT_SCHEMA }`, e na aba e na
 * palette ele vira "sem schema" (`null`).
 */
export const FLAT_SCHEMA = '';

export type CatalogNodeKind =
  | 'schema'
  | 'table'
  | 'view'
  | 'materializedView'
  | 'foreign'
  | 'partitioned';

/**
 * Situação das relações de um schema: `unloaded` = só o nome é conhecido;
 * `stale` = o banco mudou e a recarga ainda não chegou (o conteúdo antigo segue
 * visível).
 */
export type CatalogLoadState = 'unloaded' | 'loaded' | 'stale';

export interface CatalogNode {
  name: string;
  kind: CatalogNodeKind;
  /** Filhos diretos (tabelas de um schema, partições de um pai); null = ainda não se sabe */
  childCount: number | null;
  /** Só em schemas */
  state: CatalogLoadState | null;
  /** Só em schemas: quanto este tenant difere do formato dominante */
  drift: SchemaDrift | null;
}

/** Relações que faltam e que sobram em relação ao formato dominante */
export interface SchemaDrift {
  missing: number;
  extra: number;
}

/**
 * Grupo da árvore agrupada por formato: o molde dos tenants (`dominant`), as
 * variações dele (`variant`, com o que falta e o que sobra) e o resto
 * (`other`: `public`, auditoria, schemas ainda não carregados).
 */
export interface ShapeGroup {
  /** Estável enquanto o conteúdo do formato não mudar; `other` para o resto */
  key: string;
  role: 'dominant' | 'variant' | 'other';
  /** Relações do formato (null no grupo "outros") */
  tables: number | null;
  missing: string[];
  extra: string[];
  schemas: number;
}

/** Nó cujos filhos se quer listar */
export type CatalogPath =
  | { kind: 'schemas' }
  | { kind: 'schema'; schema: string }
  | { kind: 'partitions'; schema: string; table: string }
  /** Schemas de um grupo da árvore agrupada por formato */
  | { kind: 'shape'; key: string };

/** Uma janela da lista; `total` já considera o filtro */
export interface CatalogPage<T> {
  total: number;
  offset: number;
  items: T[];
}

export interface SchemaGroup {
  total: number;
  sample: string[];
}

/**
 * Resultado da busca. Relações iguais em vários schemas vêm agrupadas em
 * `schemas` ("orders em 5.000 schemas"); numa busca `schema.tabela` cada par
 * traz o schema em `schema`; um resultado que é o próprio schema não tem nenhum
 * dos dois.
 */
export interface CatalogSearchHit {
  serverId: number;
  database: string;
  name: string;
  kind: CatalogNodeKind;
  score: number;
  schema: string | null;
  schemas: SchemaGroup | null;
}

export type CatalogResolution =
  | { status: 'found'; schema: string; table: string; kind: CatalogNodeKind }
  | { status: 'ambiguous'; table: string; schemas: SchemaGroup }
  | { status: 'notFound' };

export interface CatalogStats {
  schemas: number;
  loaded: number;
  stale: number;
  unloaded: number;
  shapes: number;
  distinctNames: number;
  relations: number;
  approxHeapBytes: number;
}

export interface CatalogStatus {
  serverId: number;
  database: string;
  syncing: boolean;
  /** Última revalidação concluída com o banco (ms epoch) */
  fetchedAt: number | null;
  /** Conteúdo veio do disco e ainda não foi revalidado */
  fromDisk: boolean;
  error: string | null;
  serverVersion: string | null;
  stats: CatalogStats;
}

export interface DriftGroup {
  schemas: SchemaGroup;
  /** Relações do formato dominante que faltam neste grupo */
  missing: string[];
  /** Relações que este grupo tem e o dominante não */
  extra: string[];
}

export interface CatalogDriftReport {
  dominant: { tables: number; schemas: SchemaGroup } | null;
  divergent: DriftGroup[];
}

/** Progresso emitido como `CATALOG_EVENT`; o front invalida as fatias daquele database */
export type CatalogEvent = { serverId: number; database: string } & (
  | { type: 'syncing' }
  | { type: 'schemas'; added: string[]; removed: string[] }
  | { type: 'relations'; schemas: number }
  | {
      type: 'ready';
      added: string[];
      removed: string[];
      changed: string[];
      fetchedAt: number | null;
    }
  | { type: 'error'; message: string }
  | { type: 'cancelled' }
);

/**
 * Diagnóstico de um catálogo ("Copiar diagnóstico"): só contagens e tempos —
 * nenhum nome de schema, tabela, host ou usuário.
 */
export interface CatalogDiagnostics {
  appVersion: string;
  status: CatalogStatus;
  lastSync: {
    /** Quando terminou (ms epoch) */
    at: number;
    strategy: 'shapeFirst' | 'bulk';
    schemas: number;
    shapes: number | null;
    fetched: number;
    shared: number;
    added: number;
    removed: number;
    changed: number;
    layer0Ms: number;
    totalMs: number;
  } | null;
  drift: {
    dominantTables: number | null;
    dominantSchemas: number;
    divergentGroups: number;
    divergentSchemas: number;
  };
}
