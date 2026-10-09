import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { renderToString } from 'react-dom/server';
import { beforeEach, describe, expect, it } from 'vitest';
import {
  FLAT_SCHEMA,
  type CatalogNode,
  type CatalogPage,
  type CatalogStatus,
} from '@/api/types/catalog.types';
import type { Server } from '@/api/types/server.types';
import { encodeNodeId } from '@/lib/node-ref';
import { queryKeys } from '@/queries/keys';
import { CATALOG_PAGE_SIZE } from '@/queries/use-catalog';
import { useTreeStore } from '@/stores/tree-store';
import type { FlatRow } from './connection-tree.types';
import { useConnectionTree } from './use-connection-tree';

const PG = 1;
const SQLITE = 2;
const MONGO = 3;
const REDIS = 4;

const server = (id: number, dbType: Server['dbType']): Server => ({
  id,
  name: dbType === 'postgres' ? 'SaaS' : 'Notas',
  dbType,
  host: 'localhost',
  port: 5432,
  username: 'app',
  defaultDatabase: null,
  sslEnabled: false,
  connectionUri: null,
  createdAt: 0,
  scopeDatabases: null,
  scopeSchemas: null,
});

const status = (overrides: Partial<CatalogStatus> = {}): CatalogStatus => ({
  serverId: PG,
  database: 'saas',
  syncing: false,
  fetchedAt: 1,
  fromDisk: false,
  error: null,
  serverVersion: 'PostgreSQL 16.4',
  stats: {
    schemas: 5002,
    loaded: 5002,
    stale: 0,
    unloaded: 0,
    shapes: 4,
    distinctNames: 454,
    relations: 750_154,
    approxHeapBytes: 0,
  },
  ...overrides,
});

const schemaNode = (name: string): CatalogNode => ({
  name,
  kind: 'schema',
  childCount: 150,
  state: 'loaded',
  drift: null,
});

const tenant = (i: number) => `tenant_${String(i).padStart(5, '0')}`;

const page = (
  items: CatalogNode[],
  total = items.length,
): CatalogPage<CatalogNode> => ({
  total,
  offset: 0,
  items,
});

const serverNode = encodeNodeId({ serverId: PG });
const dbNode = encodeNodeId({ serverId: PG, database: 'saas' });

function client() {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false, staleTime: Infinity, gcTime: Infinity },
    },
  });
  queryClient.setQueryData(queryKeys.servers, [
    server(PG, 'postgres'),
    server(SQLITE, 'sqlite'),
    server(MONGO, 'mongodb'),
    server(REDIS, 'redis'),
  ]);
  queryClient.setQueryData(queryKeys.databases(PG), [
    { name: 'saas', sizeBytes: null },
  ]);
  queryClient.setQueryData(queryKeys.capabilities(PG), {
    hasSchemas: true,
    hasPrimaryKeys: true,
    supportsSql: true,
    supportsTransactions: true,
    supportsIndexes: true,
    browsable: true,
  });
  queryClient.setQueryData(queryKeys.catalogStatus(PG, 'saas'), status());
  return queryClient;
}

/**
 * No `renderToString` o zustand lê o estado *inicial* da store: os testes
 * preenchem os mapas iniciais no lugar (como o benchmark de src/perf).
 */
function treeState(
  expanded: string[],
  filters: [string, string][] = [],
  grouped: string[] = [],
) {
  const initial = useTreeStore.getInitialState();
  initial.expanded.clear();
  expanded.forEach(id => initial.expanded.add(id));
  initial.filters.clear();
  filters.forEach(([id, value]) => initial.filters.set(id, value));
  initial.limits.clear();
  initial.shapeGrouped.clear();
  grouped.forEach(id => initial.shapeGrouped.add(id));
}

function rows(queryClient: QueryClient): FlatRow[] {
  let result: FlatRow[] = [];
  function Probe() {
    result = useConnectionTree({ onEditServer: () => {} }).rows;
    return null;
  }
  renderToString(
    <QueryClientProvider client={queryClient}>
      <Probe />
    </QueryClientProvider>,
  );
  return result;
}

const describeRow = (row: FlatRow) =>
  row.variant === 'node'
    ? `${row.props.kind}:${row.props.name}`
    : row.variant === 'more'
      ? `more:${row.remaining}`
      : row.variant === 'filter'
        ? `filter:${row.total}`
        : `error:${row.message}`;

beforeEach(() => treeState([]));

describe('useConnectionTree com catálogo', () => {
  it('5.000 schemas: janela de 500, filtro e "carregar mais"', () => {
    treeState([serverNode, dbNode]);
    const queryClient = client();
    const first = Array.from({ length: CATALOG_PAGE_SIZE }, (_, i) =>
      schemaNode(tenant(i + 1)),
    );
    queryClient.setQueryData(
      queryKeys.catalogChildren(
        PG,
        'saas',
        { kind: 'schemas' },
        '',
        CATALOG_PAGE_SIZE,
      ),
      page(first, 5002),
    );

    const tree = rows(queryClient);
    const dbIndex = tree.findIndex(row => row.id === dbNode);
    const db = tree[dbIndex];
    expect(db.variant === 'node' && db.props.subLabel).toBe('5.002');
    expect(tree.slice(dbIndex + 1, dbIndex + 3).map(describeRow)).toEqual([
      'filter:5002',
      'schema:tenant_00001',
    ]);
    expect(tree.map(describeRow)).toContain('more:4502');
    expect(tree.findIndex(row => row.variant === 'more')).toBe(
      dbIndex + 2 + CATALOG_PAGE_SIZE,
    );
    expect(
      tree.filter(row => row.variant === 'node' && row.props.kind === 'schema'),
    ).toHaveLength(500);
  });

  it('o filtro vira a key da janela e mantém o campo', () => {
    treeState([serverNode, dbNode], [[dbNode, 'tenant_0004']]);
    const queryClient = client();
    queryClient.setQueryData(
      queryKeys.catalogChildren(
        PG,
        'saas',
        { kind: 'schemas' },
        'tenant_0004',
        CATALOG_PAGE_SIZE,
      ),
      page([schemaNode('tenant_00040'), schemaNode('tenant_00041')]),
    );

    const tree = rows(queryClient);
    const dbIndex = tree.findIndex(row => row.id === dbNode);
    expect(tree.slice(dbIndex + 1, dbIndex + 4).map(describeRow)).toEqual([
      'filter:2',
      'schema:tenant_00040',
      'schema:tenant_00041',
    ]);
    expect(tree.some(row => row.variant === 'more')).toBe(false);
  });

  it('partições aparecem dentro da tabela particionada', () => {
    const schema = encodeNodeId({
      serverId: PG,
      database: 'saas',
      schema: 'events',
    });
    const parent = encodeNodeId({
      serverId: PG,
      database: 'saas',
      schema: 'events',
      table: 'event_log',
    });
    treeState([serverNode, dbNode, schema, parent]);
    const queryClient = client();
    queryClient.setQueryData(
      queryKeys.catalogChildren(
        PG,
        'saas',
        { kind: 'schemas' },
        '',
        CATALOG_PAGE_SIZE,
      ),
      page([{ ...schemaNode('events'), childCount: 1 }]),
    );
    queryClient.setQueryData(
      queryKeys.catalogChildren(
        PG,
        'saas',
        { kind: 'schema', schema: 'events' },
        '',
        CATALOG_PAGE_SIZE,
      ),
      page([
        {
          name: 'event_log',
          kind: 'partitioned',
          childCount: 2,
          state: null,
          drift: null,
        },
      ]),
    );
    queryClient.setQueryData(
      queryKeys.catalogChildren(
        PG,
        'saas',
        { kind: 'partitions', schema: 'events', table: 'event_log' },
        '',
        CATALOG_PAGE_SIZE,
      ),
      page([
        {
          name: 'event_log_p1',
          kind: 'table',
          childCount: null,
          state: null,
          drift: null,
        },
        {
          name: 'event_log_p2',
          kind: 'table',
          childCount: null,
          state: null,
          drift: null,
        },
      ]),
    );

    const tree = rows(queryClient);
    const nodes = tree.filter(row => row.variant === 'node');
    const log = nodes.find(row => row.props.name === 'event_log')!;
    const partition = nodes.find(row => row.props.name === 'event_log_p1')!;
    expect(log.props.relationKind).toBe('partitioned');
    expect(partition.props.level).toBe(log.props.level + 1);
  });

  it('erro de sincronização sem schemas aparece com nova tentativa', () => {
    treeState([serverNode, dbNode]);
    const queryClient = client();
    queryClient.setQueryData(
      queryKeys.catalogStatus(PG, 'saas'),
      status({
        error: 'Connection error: timeout',
        stats: { ...status().stats, schemas: 0 },
      }),
    );
    queryClient.setQueryData(
      queryKeys.catalogChildren(
        PG,
        'saas',
        { kind: 'schemas' },
        '',
        CATALOG_PAGE_SIZE,
      ),
      page([]),
    );

    const tree = rows(queryClient);
    const dbIndex = tree.findIndex(row => row.id === dbNode);
    expect(describeRow(tree[dbIndex + 1])).toBe(
      'error:Connection error: timeout',
    );
  });

  it('sem schema (SQLite): tabelas direto sob o database, do schema sem nome', () => {
    const sqliteServer = encodeNodeId({ serverId: SQLITE });
    const sqliteDb = encodeNodeId({ serverId: SQLITE, database: 'main' });
    treeState([sqliteServer, sqliteDb]);
    const queryClient = client();
    queryClient.setQueryData(queryKeys.databases(SQLITE), [
      { name: 'main', sizeBytes: null },
    ]);
    queryClient.setQueryData(
      queryKeys.catalogStatus(SQLITE, 'main'),
      status({ serverId: SQLITE, database: 'main' }),
    );
    queryClient.setQueryData(
      queryKeys.catalogChildren(
        SQLITE,
        'main',
        { kind: 'schema', schema: FLAT_SCHEMA },
        '',
        CATALOG_PAGE_SIZE,
      ),
      page([
        {
          name: 'notas',
          kind: 'table',
          childCount: null,
          state: null,
          drift: null,
        },
        {
          name: 'resumo',
          kind: 'view',
          childCount: null,
          state: null,
          drift: null,
        },
      ]),
    );

    const tree = rows(queryClient);
    const dbIndex = tree.findIndex(row => row.id === sqliteDb);
    expect(tree.slice(dbIndex + 1, dbIndex + 3).map(describeRow)).toEqual([
      'table:notas',
      'table:resumo',
    ]);
    const notas = tree[dbIndex + 1];
    // A aba da tabela não tem schema
    expect(notas.id).toBe(
      encodeNodeId({ serverId: SQLITE, database: 'main', table: 'notas' }),
    );
    expect(notas.variant === 'node' && notas.props.level).toBe(2);
    const resumo = tree[dbIndex + 2];
    expect(resumo.variant === 'node' && resumo.props.relationKind).toBe('view');
  });

  it('5.000 coleções (Mongo): filtro e janela direto sob o database', () => {
    const mongoServer = encodeNodeId({ serverId: MONGO });
    const mongoDb = encodeNodeId({ serverId: MONGO, database: 'tenants' });
    treeState([mongoServer, mongoDb]);
    const queryClient = client();
    queryClient.setQueryData(queryKeys.databases(MONGO), [
      { name: 'tenants', sizeBytes: null },
    ]);
    queryClient.setQueryData(
      queryKeys.catalogStatus(MONGO, 'tenants'),
      status({ serverId: MONGO, database: 'tenants' }),
    );
    const collections = Array.from(
      { length: CATALOG_PAGE_SIZE },
      (_, i): CatalogNode => ({
        name: `t${String(i).padStart(5, '0')}_orders`,
        kind: 'table',
        childCount: null,
        state: null,
        drift: null,
      }),
    );
    queryClient.setQueryData(
      queryKeys.catalogChildren(
        MONGO,
        'tenants',
        { kind: 'schema', schema: FLAT_SCHEMA },
        '',
        CATALOG_PAGE_SIZE,
      ),
      page(collections, 5000),
    );

    const tree = rows(queryClient);
    const dbIndex = tree.findIndex(row => row.id === mongoDb);
    const db = tree[dbIndex];
    expect(db.variant === 'node' && db.props.subLabel).toBe('5.000');
    const filter = tree[dbIndex + 1];
    expect(filter.variant === 'filter' && filter.placeholder).toBe(
      'Filtrar coleções',
    );
    expect(tree.map(describeRow)).toContain('more:4500');
  });

  it('nível do servidor: muitos databases ganham filtro e janela', () => {
    const pgServer = encodeNodeId({ serverId: PG });
    const databases = Array.from({ length: 2000 }, (_, i) => ({
      name: `tenantdb_${String(i + 1).padStart(4, '0')}`,
      sizeBytes: null,
    }));

    treeState([pgServer]);
    const queryClient = client();
    queryClient.setQueryData(queryKeys.databases(PG), databases);
    let tree = rows(queryClient);
    const serverIndex = tree.findIndex(row => row.id === pgServer);
    const serverRow = tree[serverIndex];
    expect(serverRow.variant === 'node' && serverRow.props.subLabel).toBe(
      '2.000',
    );
    expect(describeRow(tree[serverIndex + 1])).toBe('filter:2000');
    expect(
      tree.filter(
        row => row.variant === 'node' && row.props.kind === 'database',
      ),
    ).toHaveLength(CATALOG_PAGE_SIZE);
    expect(tree.map(describeRow)).toContain('more:1500');

    // Filtro local, sem ir ao backend: só os nomes que contêm o texto
    treeState([pgServer], [[pgServer, 'DB_012']]);
    tree = rows(queryClient);
    const names = tree
      .filter(row => row.variant === 'node' && row.props.kind === 'database')
      .map(row => (row.variant === 'node' ? row.props.name : ''));
    expect(names).toHaveLength(10);
    expect(names.every(name => name.startsWith('tenantdb_012'))).toBe(true);
    expect(tree.some(row => row.variant === 'more')).toBe(false);
  });

  it('banco sem catálogo (Redis) segue com a estrutura inteira', () => {
    const redisServer = encodeNodeId({ serverId: REDIS });
    const redisDb = encodeNodeId({ serverId: REDIS, database: '0' });
    treeState([redisServer, redisDb]);
    const queryClient = client();
    queryClient.setQueryData(queryKeys.databases(REDIS), [
      { name: '0', sizeBytes: null },
    ]);
    queryClient.setQueryData(queryKeys.capabilities(REDIS), {
      hasSchemas: false,
      hasPrimaryKeys: true,
      supportsSql: false,
      supportsTransactions: false,
      supportsIndexes: false,
      browsable: true,
    });
    queryClient.setQueryData(queryKeys.structure(REDIS, '0'), {
      schemas: [
        {
          name: '0',
          tables: [{ name: 'user:*', tableType: 'table', sizeBytes: null }],
        },
      ],
      fetchedAt: 1,
    });

    expect(rows(queryClient).map(describeRow)).toContain('table:user:*');
  });

  it('lista simples: tenant fora do molde leva o aviso no lugar da contagem', () => {
    treeState([serverNode, dbNode]);
    const queryClient = client();
    queryClient.setQueryData(
      queryKeys.catalogChildren(
        PG,
        'saas',
        { kind: 'schemas' },
        '',
        CATALOG_PAGE_SIZE,
      ),
      page([
        schemaNode(tenant(99)),
        { ...schemaNode(tenant(100)), drift: { missing: 3, extra: 0 } },
      ]),
    );

    const tree = rows(queryClient);
    const behind = tree.find(
      row =>
        row.id ===
        encodeNodeId({ serverId: PG, database: 'saas', schema: tenant(100) }),
    )!;
    const fine = tree.find(
      row =>
        row.id ===
        encodeNodeId({ serverId: PG, database: 'saas', schema: tenant(99) }),
    )!;
    expect(behind.variant === 'node' && behind.props.badge?.label).toBe('−3');
    expect(behind.variant === 'node' && behind.props.subLabel).toBeUndefined();
    expect(fine.variant === 'node' && fine.props.badge).toBeUndefined();
    expect(fine.variant === 'node' && fine.props.subLabel).toBe('150');

    // Com formato repetido, o database oferece agrupar
    const db = tree.find(row => row.id === dbNode)!;
    const labels =
      db.variant === 'node'
        ? db.props.actions?.map(action => action.label)
        : [];
    expect(labels).toEqual(
      expect.arrayContaining(['Agrupar por formato', 'Copiar diagnóstico']),
    );
  });

  it('agrupado por formato: grupos e, aberto, os schemas do grupo', () => {
    const variantNode = encodeNodeId({
      serverId: PG,
      database: 'saas',
      shape: 'b2',
    });
    treeState([serverNode, dbNode, variantNode], [], [dbNode]);
    const queryClient = client();
    queryClient.setQueryData(queryKeys.catalogShapes(PG, 'saas'), [
      {
        key: 'a1',
        role: 'dominant',
        tables: 150,
        missing: [],
        extra: [],
        schemas: 4950,
      },
      {
        key: 'b2',
        role: 'variant',
        tables: 147,
        missing: ['x', 'y', 'z'],
        extra: [],
        schemas: 50,
      },
      {
        key: 'other',
        role: 'other',
        tables: null,
        missing: [],
        extra: [],
        schemas: 2,
      },
    ]);
    queryClient.setQueryData(
      queryKeys.catalogChildren(
        PG,
        'saas',
        { kind: 'shape', key: 'b2' },
        '',
        CATALOG_PAGE_SIZE,
      ),
      page([{ ...schemaNode(tenant(100)), drift: { missing: 3, extra: 0 } }]),
    );

    const tree = rows(queryClient);
    const dbIndex = tree.findIndex(row => row.id === dbNode);
    expect(tree.slice(dbIndex + 1, dbIndex + 5).map(describeRow)).toEqual([
      'shape:Formato principal',
      'shape:Variação',
      `schema:${tenant(100)}`,
      'shape:Outros schemas',
    ]);
    const variant = tree[dbIndex + 2];
    expect(variant.variant === 'node' && variant.props.badge?.label).toBe('−3');
    // Dentro do grupo o aviso não se repete em cada schema
    const inside = tree[dbIndex + 3];
    expect(inside.variant === 'node' && inside.props.badge).toBeUndefined();
    expect(inside.variant === 'node' && inside.props.level).toBe(3);
    // Sem lista simples enquanto agrupado: a contagem vem do estado do catálogo
    const db = tree[dbIndex];
    expect(db.variant === 'node' && db.props.subLabel).toBe('5.002');
    const actions =
      db.variant === 'node'
        ? db.props.actions?.map(action => action.label)
        : [];
    expect(actions).toContain('Lista simples');
  });

  it('agrupado sem formato repetido cai na lista simples', () => {
    treeState([serverNode, dbNode], [], [dbNode]);
    const queryClient = client();
    queryClient.setQueryData(queryKeys.catalogShapes(PG, 'saas'), []);
    queryClient.setQueryData(
      queryKeys.catalogChildren(
        PG,
        'saas',
        { kind: 'schemas' },
        '',
        CATALOG_PAGE_SIZE,
      ),
      page([schemaNode('public')]),
    );
    expect(rows(queryClient).map(describeRow)).toContain('schema:public');
  });
});
