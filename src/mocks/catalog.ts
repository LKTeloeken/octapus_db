import {
  CATALOG_EVENT,
  FLAT_SCHEMA,
  type CatalogDriftReport,
  type CatalogEvent,
  type CatalogNode,
  type CatalogNodeKind,
  type CatalogPage,
  type CatalogPath,
  type CatalogResolution,
  type CatalogSearchHit,
  type CatalogStatus,
  type CatalogDiagnostics,
  type SchemaDrift,
  type ShapeGroup,
} from '@/api/types/catalog.types';
import type { TableType } from '@/api/types/structure.types';
import { fuzzyMatch } from '@/lib/fuzzy';
import { requireDatabase, requireServerEntry, type MockDatabase } from './data';
import {
  SAAS_PUBLIC_TABLES,
  TENANT_COUNT,
  tenantIndex,
  tenantSchema,
  tenantTables,
} from './data/tenants';
import { mockEmit } from './events';
import { parseScope } from './scope';
import { useMockStore } from './mock-store';

/**
 * Catálogo de metadados simulado (o `CatalogService` do backend, em pequeno):
 * sincroniza em camadas com eventos, carrega na hora um schema pedido antes da
 * sincronização chegar, busca agrupando relações iguais e resolve nomes. A
 * forma de cada database vem do dataset; o `saas` tem 5.000 schemas gerados.
 */

interface Relation {
  name: string;
  kind: CatalogNodeKind;
  parent: string | null;
}

/** Nome de relação → tipo e em quais schemas aparece */
interface NameEntry {
  kind: CatalogNodeKind;
  schemas: string[];
}

interface Shape {
  /** Em ordem de bytes, como o `ORDER BY nspname` */
  schemas: string[];
  relations: (schema: string) => Relation[] | undefined;
  /** Montado na primeira busca (varre todos os schemas uma vez) */
  names?: Map<string, NameEntry>;
  /** Montado no primeiro pedido de grupos por formato */
  grouping?: Grouping;
}

/** Os formatos do database: o molde, as variações e o resto */
interface Grouping {
  groups: ShapeGroup[];
  /** Schema → grupo e, nas variações, a diferença para o molde */
  bySchema: Map<string, { key: string; drift: SchemaDrift | null }>;
}

const KIND: Record<TableType, CatalogNodeKind> = {
  table: 'table',
  view: 'view',
  materializedview: 'materializedView',
  foreign: 'foreign',
};

const byteOrder = (a: string, b: string) => (a < b ? -1 : a > b ? 1 : 0);
const sortByName = (relations: Relation[]) =>
  relations.sort((a, b) => byteOrder(a.name, b.name));
const sleep = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));

const shapes = new WeakMap<MockDatabase, Shape>();

/** `flat`: Mongo e SQLite, tudo no schema sem nome (`FLAT_SCHEMA`) */
function shapeOf(db: MockDatabase, flat: boolean): Shape {
  const cached = shapes.get(db);
  if (cached) return cached;

  let shape: Shape;
  if (db.generated === 'tenants') {
    const asRelations = (names: readonly string[]) =>
      sortByName(
        names.map(name => ({ name, kind: 'table' as const, parent: null })),
      );
    const publicRelations = asRelations(SAAS_PUBLIC_TABLES);
    const full = asRelations(tenantTables(1));
    const behind = asRelations(tenantTables(100));

    shape = {
      schemas: [
        'public',
        ...Array.from({ length: TENANT_COUNT }, (_, i) => tenantSchema(i + 1)),
      ].sort(byteOrder),
      relations: schema => {
        if (schema === 'public') return publicRelations;
        const index = tenantIndex(schema);
        if (index === null) return undefined;
        return index % 100 === 0 ? behind : full;
      },
    };
  } else {
    const bySchema = new Map<string, Relation[]>();
    for (const table of db.tables) {
      const list = bySchema.get(flat ? FLAT_SCHEMA : table.schema) ?? [];
      list.push({
        name: table.name,
        kind: KIND[table.tableType],
        parent: null,
      });
      bySchema.set(flat ? FLAT_SCHEMA : table.schema, list);
    }
    bySchema.forEach(sortByName);
    shape = {
      schemas: Array.from(bySchema.keys()).sort(byteOrder),
      relations: schema => bySchema.get(schema),
    };
  }

  shapes.set(db, shape);
  return shape;
}

const scopedShapes = new WeakMap<Shape, Map<string, Shape>>();

/** Escopo salvo de schemas: o que fica de fora some do catálogo */
function scoped(shape: Shape, rules: string | null): Shape {
  if (!rules?.trim()) return shape;
  let byRules = scopedShapes.get(shape);
  if (!byRules) {
    byRules = new Map();
    scopedShapes.set(shape, byRules);
  }
  let narrowed = byRules.get(rules);
  if (!narrowed) {
    const allows = parseScope(rules);
    narrowed = {
      schemas: shape.schemas.filter(allows),
      relations: schema =>
        allows(schema) ? shape.relations(schema) : undefined,
    };
    byRules.set(rules, narrowed);
  }
  return narrowed;
}

/** Hash curto do conteúdo (como a chave de formato do backend) */
function contentKey(relations: Relation[]): string {
  let hash = 0x811c9dc5;
  for (const relation of relations) {
    for (const char of `${relation.name}:${relation.kind},`) {
      hash = Math.imul(hash ^ char.charCodeAt(0), 0x01000193) >>> 0;
    }
  }
  return hash.toString(16).padStart(8, '0');
}

/** Mesmo critério do backend: Jaccard ≥ 0,5 com o molde é variação */
function groupingOf(shape: Shape): Grouping {
  if (shape.grouping) return shape.grouping;
  const members = new Map<
    string,
    { relations: Relation[]; schemas: string[] }
  >();
  for (const schema of shape.schemas) {
    const relations = shape.relations(schema) ?? [];
    const key = contentKey(relations);
    const entry = members.get(key);
    if (entry) entry.schemas.push(schema);
    else members.set(key, { relations, schemas: [schema] });
  }

  const ranked = Array.from(members.entries()).sort(
    (a, b) => b[1].schemas.length - a[1].schemas.length,
  );
  const grouping: Grouping = { groups: [], bySchema: new Map() };
  const [dominantKey, dominant] = ranked[0] ?? [];
  if (!dominant || dominant.schemas.length < 2) {
    shape.grouping = grouping;
    return grouping;
  }

  const reference = new Set(dominant.relations.map(r => r.name));
  grouping.groups.push({
    key: dominantKey!,
    role: 'dominant',
    tables: dominant.relations.length,
    missing: [],
    extra: [],
    schemas: dominant.schemas.length,
  });
  dominant.schemas.forEach(schema =>
    grouping.bySchema.set(schema, { key: dominantKey!, drift: null }),
  );

  const others: string[] = [];
  for (const [key, entry] of ranked.slice(1)) {
    const list = entry.relations.map(r => r.name);
    const names = new Set(list);
    const referenceList = dominant.relations.map(r => r.name);
    const shared = list.filter(name => reference.has(name)).length;
    const union = names.size + referenceList.length - shared;
    if (union === 0 || shared / union < 0.5) {
      others.push(...entry.schemas);
      continue;
    }
    const missing = referenceList
      .filter(name => !names.has(name))
      .sort(byteOrder);
    const extra = list.filter(name => !reference.has(name)).sort(byteOrder);
    grouping.groups.push({
      key,
      role: 'variant',
      tables: entry.relations.length,
      missing,
      extra,
      schemas: entry.schemas.length,
    });
    const drift = { missing: missing.length, extra: extra.length };
    entry.schemas.forEach(schema =>
      grouping.bySchema.set(schema, { key, drift }),
    );
  }
  if (others.length > 0) {
    grouping.groups.push({
      key: 'other',
      role: 'other',
      tables: null,
      missing: [],
      extra: [],
      schemas: others.length,
    });
    others.forEach(schema =>
      grouping.bySchema.set(schema, { key: 'other', drift: null }),
    );
  }

  shape.grouping = grouping;
  return grouping;
}

function namesOf(shape: Shape): Map<string, NameEntry> {
  if (shape.names) return shape.names;
  const names = new Map<string, NameEntry>();
  for (const schema of shape.schemas) {
    for (const relation of shape.relations(schema) ?? []) {
      const entry = names.get(relation.name);
      if (entry) entry.schemas.push(schema);
      else names.set(relation.name, { kind: relation.kind, schemas: [schema] });
    }
  }
  shape.names = names;
  return names;
}

// ── Sincronização simulada ──────────────────────────────────────────────────

interface SyncState {
  /** `empty`: nada ainda; `schemas`: camada 0; `ready`: relações também */
  phase: 'empty' | 'schemas' | 'ready';
  syncing: boolean;
  fetchedAt: number | null;
}

const states = new Map<string, SyncState>();
const stateKey = (serverId: number, database: string) =>
  `${serverId}\u0000${database}`;

function context(serverId: number, database: string) {
  const entry = requireServerEntry(serverId);
  const { dbType } = entry.server;
  if (dbType === 'redis') {
    throw `Unsupported database: metadata catalog is not available for ${dbType}`;
  }
  const db = requireDatabase(entry, database);
  const empty = useMockStore.getState().emptyMode;
  const shape: Shape = empty
    ? { schemas: [], relations: () => undefined }
    : scoped(
        shapeOf(db, dbType === 'mongodb' || dbType === 'sqlite'),
        entry.server.scopeSchemas,
      );

  const key = stateKey(serverId, database);
  let state = states.get(key);
  if (!state) {
    state = { phase: 'empty', syncing: false, fetchedAt: null };
    states.set(key, state);
  }
  return { shape, state };
}

function emit(
  serverId: number,
  database: string,
  event: Record<string, unknown>,
) {
  mockEmit(CATALOG_EVENT, { serverId, database, ...event } as CatalogEvent);
}

function startSync(serverId: number, database: string) {
  const { shape, state } = context(serverId, database);
  if (state.syncing) return;
  state.syncing = true;
  const firstTime = state.phase === 'empty';
  emit(serverId, database, { type: 'syncing' });

  setTimeout(() => {
    if (state.phase === 'empty') state.phase = 'schemas';
    emit(serverId, database, {
      type: 'schemas',
      added: firstTime ? shape.schemas : [],
      removed: [],
    });
  }, 300);

  setTimeout(() => {
    state.phase = 'ready';
    state.syncing = false;
    state.fetchedAt = Date.now();
    emit(serverId, database, {
      type: 'relations',
      schemas: shape.schemas.length,
    });
    emit(serverId, database, {
      type: 'ready',
      added: [],
      removed: [],
      changed: [],
      fetchedAt: state.fetchedAt,
    });
  }, 900);
}

function status(serverId: number, database: string): CatalogStatus {
  const { shape, state } = context(serverId, database);
  const known = state.phase === 'empty' ? 0 : shape.schemas.length;
  const loaded = state.phase === 'ready' ? known : 0;
  const relations =
    state.phase === 'ready'
      ? shape.schemas.reduce(
          (sum, schema) => sum + (shape.relations(schema)?.length ?? 0),
          0,
        )
      : 0;

  return {
    serverId,
    database,
    syncing: state.syncing,
    fetchedAt: state.fetchedAt,
    fromDisk: false,
    error: null,
    serverVersion: 'PostgreSQL 16.4 (mock)',
    stats: {
      schemas: known,
      loaded,
      stale: 0,
      unloaded: known - loaded,
      shapes:
        state.phase === 'ready'
          ? new Set(shape.schemas.map(shape.relations)).size
          : 0,
      distinctNames: state.phase === 'ready' ? namesOf(shape).size : 0,
      relations,
      approxHeapBytes: 0,
    },
  };
}

// ── Consultas ───────────────────────────────────────────────────────────────

const contains = (filter: string | null) => {
  const needle = filter?.trim().toLowerCase();
  return (name: string) => !needle || name.toLowerCase().includes(needle);
};

function page<T>(items: T[], offset: number, limit: number): CatalogPage<T> {
  return {
    total: items.length,
    offset,
    items: items.slice(offset, offset + limit),
  };
}

function relationNode(relations: Relation[], relation: Relation): CatalogNode {
  return {
    name: relation.name,
    kind: relation.kind,
    childCount:
      relation.kind === 'partitioned'
        ? relations.filter(other => other.parent === relation.name).length
        : null,
    state: null,
    drift: null,
  };
}

async function children(
  serverId: number,
  database: string,
  path: CatalogPath,
  filter: string | null,
  offset: number,
  limit: number,
): Promise<CatalogPage<CatalogNode>> {
  const { shape, state } = context(serverId, database);
  const keep = contains(filter);

  const ready = state.phase === 'ready';
  const schemaNode = (schema: string): CatalogNode => ({
    name: schema,
    kind: 'schema',
    childCount: ready
      ? (shape.relations(schema) ?? []).filter(r => !r.parent).length
      : null,
    state: ready ? 'loaded' : 'unloaded',
    drift: ready
      ? (groupingOf(shape).bySchema.get(schema)?.drift ?? null)
      : null,
  });

  if (path.kind === 'schemas') {
    if (state.phase === 'empty') return page([], offset, limit);
    return page(shape.schemas.filter(keep).map(schemaNode), offset, limit);
  }

  if (path.kind === 'shape') {
    const grouping = groupingOf(shape);
    if (!ready || !grouping.groups.some(group => group.key === path.key)) {
      throw `Not found: ShapeSchemas("${path.key}") in ${database}`;
    }
    const members = shape.schemas.filter(
      schema => grouping.bySchema.get(schema)?.key === path.key && keep(schema),
    );
    return page(members.map(schemaNode), offset, limit);
  }

  const relations = shape.relations(path.schema);
  if (!relations) throw `Not found: schema "${path.schema}" in ${database}`;
  // A sincronização ainda não chegou: carga prioritária só deste schema
  if (state.phase !== 'ready') await sleep(150);

  const wanted =
    path.kind === 'schema'
      ? relations.filter(r => !r.parent)
      : relations.filter(r => r.parent === path.table);
  return page(
    wanted.filter(r => keep(r.name)).map(r => relationNode(relations, r)),
    offset,
    limit,
  );
}

/** Catálogos abertos (ou o pedido), como o `search_all` do backend */
function searchTargets(scope?: { serverId: number; database: string }) {
  if (scope) return [scope];
  return Array.from(states.entries())
    .filter(([, state]) => state.phase !== 'empty')
    .map(([key]) => {
      const [serverId, database] = key.split('\u0000');
      return { serverId: Number(serverId), database };
    });
}

/** Relações de um schema exato (o fixado na palette), como `search_in_schema` */
function searchInSchema(
  schemaName: string,
  query: string,
  limit: number,
  scope?: { serverId: number; database: string },
): CatalogSearchHit[] {
  const hits: CatalogSearchHit[] = [];
  for (const { serverId, database } of searchTargets(scope)) {
    const { shape } = context(serverId, database);
    const schema =
      shape.schemas.find(name => name === schemaName) ??
      shape.schemas.find(
        name => name.toLowerCase() === schemaName.toLowerCase(),
      );
    if (schema === undefined) continue;
    for (const relation of shape.relations(schema) ?? []) {
      const match = query ? fuzzyMatch(query, relation.name) : null;
      if (match && !match.matched) continue;
      hits.push({
        serverId,
        database,
        name: relation.name,
        kind: relation.kind,
        score: match?.score ?? 0,
        schema,
        schemas: null,
      });
    }
  }
  return hits
    .sort(
      (a, b) =>
        b.score - a.score ||
        a.name.length - b.name.length ||
        byteOrder(a.name, b.name),
    )
    .slice(0, limit);
}

function search(
  query: string,
  limit: number,
  scope?: { serverId: number; database: string },
  schema?: string | null,
): CatalogSearchHit[] {
  const trimmed = query.trim();
  if (schema != null) return searchInSchema(schema, trimmed, limit, scope);
  if (!trimmed) return [];
  const hits: CatalogSearchHit[] = [];

  for (const { serverId, database } of searchTargets(scope)) {
    const { shape } = context(serverId, database);
    const base = { serverId, database };
    const dot = trimmed.indexOf('.');

    if (dot === -1) {
      namesOf(shape).forEach((entry, name) => {
        const match = fuzzyMatch(trimmed, name);
        if (!match.matched) return;
        hits.push({
          ...base,
          name,
          kind: entry.kind,
          score: match.score,
          schema: null,
          schemas: {
            total: entry.schemas.length,
            sample: entry.schemas.slice(0, 3),
          },
        });
      });
      for (const schema of shape.schemas) {
        const match = fuzzyMatch(trimmed, schema);
        if (match.matched) {
          hits.push({
            ...base,
            name: schema,
            kind: 'schema',
            score: match.score,
            schema: null,
            schemas: null,
          });
        }
      }
      continue;
    }

    const schemaQuery = trimmed.slice(0, dot);
    const tableQuery = trimmed.slice(dot + 1);
    if (!schemaQuery && !tableQuery) continue;
    for (const schema of shape.schemas) {
      const schemaMatch = schemaQuery ? fuzzyMatch(schemaQuery, schema) : null;
      if (schemaMatch && !schemaMatch.matched) continue;
      for (const relation of shape.relations(schema) ?? []) {
        const tableMatch = tableQuery
          ? fuzzyMatch(tableQuery, relation.name)
          : null;
        if (tableMatch && !tableMatch.matched) continue;
        hits.push({
          ...base,
          name: relation.name,
          kind: relation.kind,
          score: (schemaMatch?.score ?? 0) + (tableMatch?.score ?? 0),
          schema,
          schemas: null,
        });
      }
      // Sem limite aqui o "t.inv" do saas montaria 60 mil pares
      if (hits.length > limit * 20) break;
    }
  }

  return hits
    .sort(
      (a, b) =>
        b.score - a.score ||
        a.name.length - b.name.length ||
        byteOrder(a.name, b.name),
    )
    .slice(0, limit);
}

function resolve(
  serverId: number,
  database: string,
  table: string,
  searchPath: string[],
): CatalogResolution {
  const { shape } = context(serverId, database);
  const names = namesOf(shape);
  const lower = table.toLowerCase();
  const candidates = [
    ...(names.has(table) ? [table] : []),
    ...Array.from(names.keys()).filter(
      name => name !== table && name.toLowerCase() === lower,
    ),
  ];

  for (const schema of searchPath) {
    const relations = shape.relations(schema) ?? [];
    for (const candidate of candidates) {
      const found = relations.find(relation => relation.name === candidate);
      if (found)
        return { status: 'found', schema, table: candidate, kind: found.kind };
    }
  }
  for (const candidate of candidates) {
    const entry = names.get(candidate)!;
    if (entry.schemas.length === 1) {
      return {
        status: 'found',
        schema: entry.schemas[0],
        table: candidate,
        kind: entry.kind,
      };
    }
    return {
      status: 'ambiguous',
      table: candidate,
      schemas: {
        total: entry.schemas.length,
        sample: entry.schemas.slice(0, 3),
      },
    };
  }
  return { status: 'notFound' };
}

async function complete(
  serverId: number,
  database: string,
  schema: string | null,
  prefix: string,
  limit: number,
): Promise<CatalogNode[]> {
  const { shape, state } = context(serverId, database);
  const starts = (name: string) =>
    name.toLowerCase().startsWith(prefix.toLowerCase());

  if (schema === null) {
    return shape.schemas
      .filter(starts)
      .slice(0, limit)
      .map(name => ({
        name,
        kind: 'schema',
        childCount: null,
        state: null,
        drift: null,
      }));
  }
  const relations = shape.relations(schema);
  if (!relations) return [];
  if (state.phase !== 'ready') await sleep(150);
  return relations
    .filter(relation => starts(relation.name))
    .slice(0, limit)
    .map(relation => relationNode(relations, relation));
}

function drift(serverId: number, database: string): CatalogDriftReport {
  const { shape } = context(serverId, database);
  const db = requireDatabase(requireServerEntry(serverId), database);
  if (db.generated !== 'tenants' || shape.schemas.length === 0) {
    return { dominant: null, divergent: [] };
  }
  const tenants = shape.schemas.filter(schema => tenantIndex(schema) !== null);
  const behind = tenants.filter(schema => tenantIndex(schema)! % 100 === 0);
  const full = tenants.filter(schema => tenantIndex(schema)! % 100 !== 0);
  const missing = tenantTables(1).filter(
    name => !tenantTables(100).includes(name),
  );

  return {
    dominant: {
      tables: tenantTables(1).length,
      schemas: { total: full.length, sample: full.slice(0, 5) },
    },
    divergent: [
      {
        schemas: { total: behind.length, sample: behind.slice(0, 5) },
        missing: [...missing].sort(byteOrder),
        extra: [],
      },
    ],
  };
}

// ── API do mock (chamada pelos handlers) ────────────────────────────────────

type Args = Record<string, unknown>;

export const mockCatalog = {
  open({ serverId, database }: Args): CatalogStatus {
    const id = serverId as number;
    const db = database as string;
    const { state } = context(id, db);
    if (
      state.phase !== 'ready' ||
      !state.fetchedAt ||
      Date.now() - state.fetchedAt > 60_000
    ) {
      startSync(id, db);
    }
    return status(id, db);
  },

  children({ serverId, database, path, filter, offset, limit }: Args) {
    return children(
      serverId as number,
      database as string,
      path as CatalogPath,
      (filter as string | null) ?? null,
      (offset as number) ?? 0,
      limit as number,
    );
  },

  search({
    query,
    limit,
    serverId,
    database,
    schema,
  }: Args): CatalogSearchHit[] {
    const scope =
      serverId != null && database != null
        ? { serverId: serverId as number, database: database as string }
        : undefined;
    return search(
      query as string,
      limit as number,
      scope,
      schema as string | null,
    );
  },

  resolve({ serverId, database, table, searchPath }: Args): CatalogResolution {
    return resolve(
      serverId as number,
      database as string,
      table as string,
      searchPath as string[],
    );
  },

  complete({ serverId, database, schema, prefix, limit }: Args) {
    return complete(
      serverId as number,
      database as string,
      (schema as string | null) ?? null,
      prefix as string,
      limit as number,
    );
  },

  drift({ serverId, database }: Args): CatalogDriftReport {
    return drift(serverId as number, database as string);
  },

  async refresh({ serverId, database, schema }: Args): Promise<CatalogStatus> {
    const id = serverId as number;
    const db = database as string;
    if (schema) {
      await sleep(150);
      emit(id, db, { type: 'relations', schemas: 1 });
    } else {
      startSync(id, db);
    }
    return status(id, db);
  },

  status({ serverId, database }: Args): CatalogStatus | null {
    return states.has(stateKey(serverId as number, database as string))
      ? status(serverId as number, database as string)
      : null;
  },

  shapes({ serverId, database }: Args): ShapeGroup[] {
    const { shape, state } = context(serverId as number, database as string);
    return state.phase === 'ready' ? groupingOf(shape).groups : [];
  },

  diagnostics({ serverId, database }: Args): CatalogDiagnostics {
    const id = serverId as number;
    const db = database as string;
    const { shape, state } = context(id, db);
    const current = status(id, db);
    const groups = state.phase === 'ready' ? groupingOf(shape).groups : [];
    const dominant = groups.find(group => group.role === 'dominant');
    const variants = groups.filter(group => group.role === 'variant');
    return {
      appVersion: '0.1.0-mock',
      status: current,
      lastSync: state.fetchedAt
        ? {
            at: state.fetchedAt,
            strategy: 'shapeFirst',
            schemas: current.stats.schemas,
            shapes: current.stats.shapes,
            fetched: current.stats.shapes,
            shared: current.stats.schemas - current.stats.shapes,
            added: 0,
            removed: 0,
            changed: 0,
            layer0Ms: 300,
            totalMs: 900,
          }
        : null,
      drift: {
        dominantTables: dominant?.tables ?? null,
        dominantSchemas: dominant?.schemas ?? 0,
        divergentGroups: variants.length,
        divergentSchemas: variants.reduce(
          (sum, group) => sum + group.schemas,
          0,
        ),
      },
    };
  },

  /** Servidor editado ou removido: os catálogos dele somem (como no backend) */
  forgetServer(serverId: number): void {
    for (const key of Array.from(states.keys())) {
      if (key.startsWith(`${serverId}\u0000`)) states.delete(key);
    }
  },

  relationSize({ table }: Args): number {
    return 16_384 + (table as string).length * 8_192;
  },

  /** Volta tudo ao estado de "nunca aberto" (reset do dataset no painel) */
  reset(): void {
    states.clear();
  },
};
