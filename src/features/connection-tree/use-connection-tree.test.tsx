import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { renderToString } from 'react-dom/server';
import { beforeEach, describe, expect, it } from 'vitest';
import {
  FLAT_SCHEMA,
  type CatalogNode,
  type CatalogObject,
  type CatalogPage,
  type CatalogStatus,
} from '@/api/types/catalog.types';
import type { DatabaseType, Server } from '@/api/types/server.types';
import { encodeNodeId, groupNodeId } from '@/lib/node-ref';
import { CAPABILITIES } from '@/mocks/data';
import { useTabsStore } from '@/stores/tabs-store';
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
  sslMode: 'disable',
  sslRootCert: null,
  connectionUri: null,
  createdAt: 0,
  scopeDatabases: null,
  scopeSchemas: null,
  groupId: null,
  assistantSampleValues: false,
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

const objectPage = (
  items: CatalogObject[],
  total = items.length,
): CatalogPage<CatalogObject> => ({ total, offset: 0, items });

const serverNode = encodeNodeId({ serverId: PG });
const dbNode = encodeNodeId({ serverId: PG, database: 'saas' });

/** A tabela do `get_db_capabilities`, a mesma do mock */
const DB_CAPABILITIES = (Object.keys(CAPABILITIES) as DatabaseType[]).map(
  dbType => ({ dbType, capabilities: CAPABILITIES[dbType] }),
);

function client({ withCapabilities = true } = {}) {
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
  if (withCapabilities) {
    queryClient.setQueryData(queryKeys.dbCapabilities, DB_CAPABILITIES);
  }
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
  initial.collapsedGroups.clear();
}

function rows(queryClient: QueryClient): FlatRow[] {
  let result: FlatRow[] = [];
  function Probe() {
    result = useConnectionTree({
      onEditServer: () => {},
      onCreateServer: () => {},
    }).rows;
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
        : row.variant === 'empty'
          ? `empty:${row.message}`
          : `error:${row.message}`;

/** As linhas abaixo do nó `id`, até o próximo servidor */
function below(tree: FlatRow[], id: string) {
  const at = tree.findIndex(row => row.id === id);
  const end = tree.findIndex(
    (row, i) => i > at && row.variant === 'node' && row.props.kind === 'server',
  );
  return tree.slice(at + 1, end === -1 ? undefined : end);
}

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

  it('partições numa pasta da tabela particionada, depois de colunas e índices', () => {
    const schema = encodeNodeId({
      serverId: PG,
      database: 'saas',
      schema: 'events',
    });
    const tables = encodeNodeId({
      serverId: PG,
      database: 'saas',
      schema: 'events',
      folder: 'tables',
    });
    const parent = encodeNodeId({
      serverId: PG,
      database: 'saas',
      schema: 'events',
      table: 'event_log',
    });
    const partitions = encodeNodeId({
      serverId: PG,
      database: 'saas',
      schema: 'events',
      table: 'event_log',
      folder: 'partitions',
    });
    treeState([serverNode, dbNode, schema, tables, parent, partitions]);
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
        { kind: 'tables', schema: 'events' },
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
    const at = tree.findIndex(row => row.id === parent);
    expect(tree.slice(at, at + 7).map(describeRow)).toEqual([
      'table:event_log',
      'folder:Colunas',
      'folder:Índices',
      'folder:Triggers',
      'folder:Chaves estrangeiras',
      'folder:Partições',
      'table:event_log_p1',
    ]);
    const nodes = tree.filter(row => row.variant === 'node');
    const log = nodes.find(row => row.props.name === 'event_log')!;
    const partition = nodes.find(row => row.props.name === 'event_log_p1')!;
    expect(log.props.relationKind).toBe('partitioned');
    expect(partition.props.level).toBe(log.props.level + 2);
    const folderRow = tree[at + 5];
    expect(folderRow.variant === 'node' && folderRow.props.subLabel).toBe('2');
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
    // Falha de conexão: os detalhes levam a "Editar servidor"
    const error = tree[dbIndex + 1];
    expect(error.variant === 'error' && typeof error.onEdit).toBe('function');
  });

  it('sem schema (SQLite): pastas direto sob o database, do schema sem nome', () => {
    const sqliteServer = encodeNodeId({ serverId: SQLITE });
    const sqliteDb = encodeNodeId({ serverId: SQLITE, database: 'main' });
    const tablesFolder = encodeNodeId({
      serverId: SQLITE,
      database: 'main',
      folder: 'tables',
    });
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
        { kind: 'tables', schema: FLAT_SCHEMA },
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
      ]),
    );
    // As views moram na pasta delas, depois da pasta das tabelas
    queryClient.setQueryData(
      queryKeys.catalogFolders(SQLITE, 'main', FLAT_SCHEMA),
      [{ kind: 'view', count: 1 }],
    );
    queryClient.setQueryData(
      queryKeys.catalogObjects(
        SQLITE,
        'main',
        FLAT_SCHEMA,
        'view',
        null,
        '',
        CATALOG_PAGE_SIZE,
      ),
      objectPage([{ kind: 'view', name: 'resumo' }]),
    );
    const viewsFolder = encodeNodeId({
      serverId: SQLITE,
      database: 'main',
      folder: 'view',
    });

    let tree = rows(queryClient);
    let dbIndex = tree.findIndex(row => row.id === sqliteDb);
    expect(below(tree, sqliteDb).map(describeRow)).toEqual([
      'folder:Tabelas',
      'folder:Views',
    ]);

    treeState([sqliteServer, sqliteDb, tablesFolder]);
    tree = rows(queryClient);
    dbIndex = tree.findIndex(row => row.id === sqliteDb);
    expect(below(tree, sqliteDb).map(describeRow)).toEqual([
      'folder:Tabelas',
      'table:notas',
      'folder:Views',
    ]);
    const notas = tree[dbIndex + 2];
    // A aba da tabela não tem schema
    expect(notas.id).toBe(
      encodeNodeId({ serverId: SQLITE, database: 'main', table: 'notas' }),
    );
    expect(notas.variant === 'node' && notas.props.level).toBe(3);

    // Aberta, a pasta mostra a view como relação (clique abre os dados); a
    // view aberta lista as colunas direto, sem pastas
    const resumoNode = encodeNodeId({
      serverId: SQLITE,
      database: 'main',
      table: 'resumo',
    });
    queryClient.setQueryData(queryKeys.columns(SQLITE, 'main', '', 'resumo'), [
      {
        name: 'total',
        ordinal: 1,
        dataType: 'INTEGER',
        isNullable: true,
        defaultValue: null,
        isPrimaryKey: false,
        isForeignKey: false,
      },
    ]);
    treeState([sqliteServer, sqliteDb, viewsFolder, resumoNode]);
    tree = rows(queryClient);
    dbIndex = tree.findIndex(row => row.id === sqliteDb);
    expect(below(tree, sqliteDb).map(describeRow)).toEqual([
      'folder:Tabelas',
      'folder:Views',
      'table:resumo',
      'column:total',
    ]);
    const resumo = tree[dbIndex + 3];
    expect(resumo.id).toBe(
      encodeNodeId({ serverId: SQLITE, database: 'main', table: 'resumo' }),
    );
    expect(resumo.variant === 'node' && resumo.props.relationKind).toBe('view');
    expect(resumo.variant === 'node' && resumo.props.level).toBe(3);
    expect(
      resumo.variant === 'node' &&
        resumo.props.actions?.map(action => action.label),
    ).toContain('Ver definição');
  });

  it('5.000 coleções (Mongo): filtro e janela direto sob o database', () => {
    const mongoServer = encodeNodeId({ serverId: MONGO });
    const mongoDb = encodeNodeId({ serverId: MONGO, database: 'tenants' });
    const collectionsFolder = encodeNodeId({
      serverId: MONGO,
      database: 'tenants',
      folder: 'tables',
    });
    treeState([mongoServer, mongoDb, collectionsFolder]);
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
        { kind: 'tables', schema: FLAT_SCHEMA },
        '',
        CATALOG_PAGE_SIZE,
      ),
      page(collections, 5000),
    );

    const tree = rows(queryClient);
    const dbIndex = tree.findIndex(row => row.id === mongoDb);
    const db = tree[dbIndex];
    expect(db.variant === 'node' && db.props.subLabel).toBe('5.000');
    const folderRow = tree[dbIndex + 1];
    expect(describeRow(folderRow)).toBe('folder:Coleções');
    expect(folderRow.variant === 'node' && folderRow.props.subLabel).toBe(
      '5.000',
    );
    const filter = tree[dbIndex + 2];
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

  it('sem as capabilities ainda, o Postgres não cai na estrutura inteira', () => {
    treeState([serverNode, dbNode]);
    const queryClient = client({ withCapabilities: false });
    // Se o Postgres fosse tratado como banco sem catálogo, esta estrutura
    // (a de list_schemas_with_tables) apareceria na árvore
    queryClient.setQueryData(queryKeys.structure(PG, 'saas'), {
      schemas: [
        {
          name: 'legado',
          tables: [
            { name: 'nao_deveria', tableType: 'table', sizeBytes: null },
          ],
        },
      ],
      fetchedAt: 1,
    });

    expect(rows(queryClient).map(describeRow)).not.toContain('schema:legado');
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

describe('useConnectionTree com grupos', () => {
  const rowLevel = (row: FlatRow) =>
    row.variant === 'node' ? row.props.level : row.level;

  /** O Postgres e o SQLite no grupo 10; o grupo 20 fica vazio */
  function groupedClient() {
    const queryClient = client();
    queryClient.setQueryData(queryKeys.serverGroups, [
      { id: 10, name: 'Produção', createdAt: 0 },
      { id: 20, name: 'Vazio', createdAt: 0 },
    ]);
    queryClient.setQueryData(queryKeys.servers, [
      { ...server(PG, 'postgres'), groupId: 10 },
      { ...server(SQLITE, 'sqlite'), groupId: 10 },
      server(MONGO, 'mongodb'),
      // Grupo que não existe mais (cache defasado): fica fora de grupo
      { ...server(REDIS, 'redis'), groupId: 99 },
    ]);
    return queryClient;
  }

  it('grupos primeiro, servidores do grupo um nível abaixo, subárvore inclusive', () => {
    treeState([serverNode, dbNode]);
    const queryClient = groupedClient();
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

    const tree = rows(queryClient);
    expect(tree.map(row => `${describeRow(row)}@${rowLevel(row)}`)).toEqual([
      'group:Produção@0',
      'server:SaaS@1',
      'database:saas@2',
      'schema:public@3',
      'server:Notas@1',
      'group:Vazio@0',
      'server:Notas@0',
      'server:Notas@0',
    ]);
    const empty = tree.find(row => row.id === groupNodeId(20));
    expect(empty?.variant === 'node' && empty.props.hasChildren).toBe(false);
  });

  it('grupo fechado esconde os servidores dele', () => {
    treeState([]);
    useTreeStore.getInitialState().collapsedGroups.add(10);
    const tree = rows(groupedClient());
    expect(tree.map(describeRow)).toEqual([
      'group:Produção',
      'group:Vazio',
      'server:Notas',
      'server:Notas',
    ]);
    const group = tree[0];
    expect(group.variant === 'node' && group.props.isExpanded).toBe(false);
  });
});

describe('tree-store com grupos', () => {
  it('grupo nasce aberto e expandir/colapsar usam o conjunto de fechados', () => {
    const store = useTreeStore.getState();
    const id = groupNodeId(7);
    store.collapseNode(id);
    expect(useTreeStore.getState().collapsedGroups.has(7)).toBe(true);
    expect(useTreeStore.getState().expanded.has(id)).toBe(false);
    store.toggleNode(id);
    expect(useTreeStore.getState().collapsedGroups.has(7)).toBe(false);
    store.toggleNode(id);
    store.expandNode(id);
    expect(useTreeStore.getState().collapsedGroups.has(7)).toBe(false);
  });
});

describe('useConnectionTree: pastas de tabelas, colunas, views, índices e funções', () => {
  const schema = encodeNodeId({
    serverId: PG,
    database: 'saas',
    schema: 'app',
  });
  const folder = (kind: string) =>
    encodeNodeId({
      serverId: PG,
      database: 'saas',
      schema: 'app',
      folder: kind,
    });
  const orders = encodeNodeId({
    serverId: PG,
    database: 'saas',
    schema: 'app',
    table: 'orders',
  });
  const ordersFolder = (kind: string) =>
    encodeNodeId({
      serverId: PG,
      database: 'saas',
      schema: 'app',
      table: 'orders',
      folder: kind,
    });

  function pgClient() {
    const queryClient = client();
    queryClient.setQueryData(
      queryKeys.catalogChildren(
        PG,
        'saas',
        { kind: 'schemas' },
        '',
        CATALOG_PAGE_SIZE,
      ),
      page([schemaNode('app')]),
    );
    queryClient.setQueryData(
      queryKeys.catalogChildren(
        PG,
        'saas',
        { kind: 'tables', schema: 'app' },
        '',
        CATALOG_PAGE_SIZE,
      ),
      page([
        {
          name: 'orders',
          kind: 'table',
          childCount: null,
          state: null,
          drift: null,
        },
      ]),
    );
    queryClient.setQueryData(queryKeys.catalogFolders(PG, 'saas', 'app'), [
      { kind: 'view', count: 0 },
      { kind: 'materializedView', count: 2 },
      { kind: 'function', count: null },
    ]);
    queryClient.setQueryData(queryKeys.columns(PG, 'saas', 'app', 'orders'), [
      {
        name: 'id',
        ordinal: 1,
        dataType: 'integer',
        isNullable: false,
        defaultValue: null,
        isPrimaryKey: true,
        isForeignKey: false,
      },
      {
        name: 'total',
        ordinal: 2,
        dataType: 'numeric',
        isNullable: true,
        defaultValue: null,
        isPrimaryKey: false,
        isForeignKey: false,
      },
    ]);
    return queryClient;
  }

  const objects = (
    queryClient: QueryClient,
    kind: CatalogObject['kind'],
    data: CatalogPage<CatalogObject>,
    table: string | null = null,
    server = PG,
    database = 'saas',
    schemaName = 'app',
  ) =>
    queryClient.setQueryData(
      queryKeys.catalogObjects(
        server,
        database,
        schemaName,
        kind,
        table,
        '',
        CATALOG_PAGE_SIZE,
      ),
      data,
    );

  const afterSchema = (tree: FlatRow[]) => below(tree, schema);

  it('Tabelas primeiro, depois as pastas de objetos; a que se sabe vazia não aparece', () => {
    treeState([serverNode, dbNode, schema]);
    const tree = afterSchema(rows(pgClient()));
    expect(tree.map(describeRow)).toEqual([
      'folder:Tabelas',
      'folder:Views materializadas',
      'folder:Funções',
    ]);
    const [tables, materialized, functions] = tree;
    expect(tables.variant === 'node' && tables.props.subLabel).toBe('1');
    expect(tables.variant === 'node' && tables.props.folderKind).toBe('tables');
    expect(materialized.variant === 'node' && materialized.props.subLabel).toBe(
      '2',
    );
    // Funções ainda sem contagem: só vão ao banco ao abrir
    expect(functions.variant === 'node' && functions.props.subLabel).toBe(
      undefined,
    );
  });

  it('tabela aberta: Colunas e Índices, cada um na sua pasta', () => {
    treeState([serverNode, dbNode, schema, folder('tables'), orders]);
    let tree = afterSchema(rows(pgClient()));
    expect(tree.slice(0, 4).map(describeRow)).toEqual([
      'folder:Tabelas',
      'table:orders',
      'folder:Colunas',
      'folder:Índices',
    ]);
    const columns = tree[2];
    expect(columns.variant === 'node' && columns.props.subLabel).toBe('2');
    expect(columns.variant === 'node' && columns.props.level).toBe(5);

    treeState([
      serverNode,
      dbNode,
      schema,
      folder('tables'),
      orders,
      ordersFolder('columns'),
    ]);
    tree = afterSchema(rows(pgClient()));
    expect(tree.slice(2, 6).map(describeRow)).toEqual([
      'folder:Colunas',
      'column:id',
      'column:total',
      'folder:Índices',
    ]);
    const id = tree[3];
    expect(id.variant === 'node' && id.props.subLabel).toBe('integer (PK)');
    expect(id.variant === 'node' && id.props.level).toBe(6);
  });

  it('índices da tabela em janela, com filtro, colunas e papel', () => {
    treeState([
      serverNode,
      dbNode,
      schema,
      folder('tables'),
      orders,
      ordersFolder('index'),
    ]);
    const queryClient = pgClient();
    const indexes = Array.from(
      { length: CATALOG_PAGE_SIZE },
      (_, i): CatalogObject => ({
        kind: 'index',
        name: `idx_${String(i).padStart(3, '0')}`,
        table: 'orders',
        method: 'btree',
        unique: i === 0,
        primary: false,
        columns: ['customer', 'total'],
        predicate: i === 0 ? "status <> 'void'" : null,
      }),
    );
    objects(queryClient, 'index', objectPage(indexes, 900), 'orders');

    const tree = afterSchema(rows(queryClient));
    expect(tree.slice(2, 6).map(describeRow)).toEqual([
      'folder:Colunas',
      'folder:Índices',
      'filter:900',
      'index:idx_000',
    ]);
    const folderRow = tree[3];
    expect(folderRow.variant === 'node' && folderRow.props.subLabel).toBe(
      '900',
    );
    // Dentro da tabela, o rótulo não repete a tabela
    const first = tree[5];
    expect(first.variant === 'node' && first.props.subLabel).toBe(
      '(customer, total) · btree · único · parcial',
    );
    expect(first.variant === 'node' && first.props.level).toBe(6);
    expect(tree.map(describeRow)).toContain('more:400');
    const filter = tree[4];
    expect(filter.variant === 'filter' && filter.placeholder).toBe(
      'Filtrar índices',
    );
  });

  it('triggers e chaves estrangeiras na tabela, e a FK abre a referenciada', () => {
    treeState([
      serverNode,
      dbNode,
      schema,
      folder('tables'),
      orders,
      ordersFolder('trigger'),
      ordersFolder('foreignKey'),
    ]);
    const queryClient = pgClient();
    objects(
      queryClient,
      'trigger',
      objectPage([
        {
          kind: 'trigger',
          name: 'orders_audit',
          table: 'orders',
          timing: 'BEFORE',
          events: ['UPDATE', 'DELETE'],
          function: 'app.audit',
          enabled: false,
        },
      ]),
      'orders',
    );
    objects(
      queryClient,
      'foreignKey',
      objectPage([
        {
          kind: 'foreignKey',
          name: 'orders_customer_id_fkey',
          table: 'orders',
          columns: ['customer_id'],
          refSchema: 'crm',
          refTable: 'customers',
          refColumns: ['id'],
          onDelete: 'CASCADE',
          onUpdate: 'NO ACTION',
        },
      ]),
      'orders',
    );

    const tree = afterSchema(rows(queryClient));
    expect(tree.slice(1, 8).map(describeRow)).toEqual([
      'table:orders',
      'folder:Colunas',
      'folder:Índices',
      'folder:Triggers',
      'trigger:orders_audit',
      'folder:Chaves estrangeiras',
      'foreignKey:orders_customer_id_fkey',
    ]);
    const [trigger, key] = [tree[5], tree[7]];
    expect(trigger.variant === 'node' && trigger.props.subLabel).toBe(
      'BEFORE UPDATE OR DELETE · app.audit · desligado',
    );
    expect(key.variant === 'node' && key.props.subLabel).toBe(
      '(customer_id) → customers(id) · ON DELETE CASCADE',
    );
    expect(
      key.variant === 'node' && key.props.actions?.map(action => action.label),
    ).toEqual(['Abrir tabela referenciada', 'Ver definição']);
    // Abrir a referenciada leva o schema dela, não o da tabela
    if (key.variant === 'node') key.props.actions?.[0].onSelect();
    const opened = Array.from(useTabsStore.getState().tabs.values()).pop();
    expect(opened?.kind === 'browse' && [opened.schema, opened.table]).toEqual([
      'crm',
      'customers',
    ]);
  });

  it('funções com assinatura, retorno, tipo e linguagem; sobrecargas distintas', () => {
    treeState([serverNode, dbNode, schema, folder('function')]);
    const queryClient = pgClient();
    const add = (signature: string, returns: string): CatalogObject => ({
      kind: 'function',
      name: 'add',
      signature,
      arguments: signature,
      returns,
      functionKind: 'function',
      language: 'sql',
      comment: null,
    });
    objects(
      queryClient,
      'function',
      objectPage([
        add('a integer, b integer', 'integer'),
        add('a text, b text', 'text'),
        {
          kind: 'function',
          name: 'archive',
          signature: 'days integer',
          arguments: 'days integer DEFAULT 90',
          returns: null,
          functionKind: 'procedure',
          language: 'plpgsql',
          comment: 'Arquiva os pedidos antigos',
        },
      ]),
    );

    const tree = afterSchema(rows(queryClient)).filter(
      row => row.variant === 'node' && row.props.kind === 'function',
    );
    expect(tree.map(describeRow)).toEqual([
      'function:add(a integer, b integer)',
      'function:add(a text, b text)',
      'function:archive(days integer)',
    ]);
    expect(new Set(tree.map(row => row.id)).size).toBe(3);
    const [first, , procedure] = tree;
    expect(first.variant === 'node' && first.props.subLabel).toBe(
      '→ integer · função · sql',
    );
    expect(procedure.variant === 'node' && procedure.props.subLabel).toBe(
      'procedure · plpgsql',
    );
    // O comentário só no passar do mouse
    expect(procedure.variant === 'node' && procedure.props.title).toBe(
      'archive(days integer) procedure · plpgsql\nArquiva os pedidos antigos',
    );
    expect(first.variant === 'node' && first.props.title).toBe(
      'add(a integer, b integer) → integer · função · sql',
    );
    expect(
      first.variant === 'node' &&
        first.props.actions?.map(action => action.label),
    ).toEqual(['Ver definição']);
  });

  it('pasta aberta que veio vazia continua, com o aviso dentro', () => {
    treeState([serverNode, dbNode, schema, folder('function')]);
    const queryClient = pgClient();
    objects(queryClient, 'function', objectPage([]));
    const tree = afterSchema(rows(queryClient));
    expect(tree.map(describeRow)).toEqual([
      'folder:Tabelas',
      'folder:Views materializadas',
      'folder:Funções',
      'empty:Nenhuma função',
    ]);
    const functions = tree[2];
    expect(functions.variant === 'node' && functions.props.subLabel).toBe('0');

    // O catálogo já sabendo que não há funções: a pasta fechada também fica
    queryClient.setQueryData(queryKeys.catalogFolders(PG, 'saas', 'app'), [
      { kind: 'view', count: 0 },
      { kind: 'materializedView', count: 2 },
      { kind: 'function', count: 0 },
    ]);
    treeState([serverNode, dbNode, schema]);
    expect(afterSchema(rows(queryClient)).map(describeRow)).toContain(
      'folder:Funções',
    );
  });

  it('pasta fechada não pede objetos, nem a de um schema fechado', () => {
    treeState([serverNode, dbNode, folder('function')]);
    const tree = rows(pgClient());
    expect(tree.map(describeRow)).not.toContain('folder:Funções');
  });

  it('Mongo: coleção aberta com Campos e Índices', () => {
    const mongoServer = encodeNodeId({ serverId: MONGO });
    const mongoDb = encodeNodeId({ serverId: MONGO, database: 'loja' });
    const collectionsFolder = encodeNodeId({
      serverId: MONGO,
      database: 'loja',
      folder: 'tables',
    });
    const collection = encodeNodeId({
      serverId: MONGO,
      database: 'loja',
      table: 'orders',
    });
    const inCollection = (kind: string) =>
      encodeNodeId({
        serverId: MONGO,
        database: 'loja',
        table: 'orders',
        folder: kind,
      });
    treeState([
      mongoServer,
      mongoDb,
      collectionsFolder,
      collection,
      inCollection('columns'),
      inCollection('index'),
    ]);
    const queryClient = client();
    queryClient.setQueryData(queryKeys.databases(MONGO), [
      { name: 'loja', sizeBytes: null },
    ]);
    queryClient.setQueryData(
      queryKeys.catalogStatus(MONGO, 'loja'),
      status({ serverId: MONGO, database: 'loja' }),
    );
    queryClient.setQueryData(
      queryKeys.catalogChildren(
        MONGO,
        'loja',
        { kind: 'tables', schema: FLAT_SCHEMA },
        '',
        CATALOG_PAGE_SIZE,
      ),
      page([
        {
          name: 'orders',
          kind: 'table',
          childCount: null,
          state: null,
          drift: null,
        },
      ]),
    );
    queryClient.setQueryData(
      queryKeys.catalogFolders(MONGO, 'loja', FLAT_SCHEMA),
      [{ kind: 'view', count: 0 }],
    );
    queryClient.setQueryData(queryKeys.columns(MONGO, 'loja', '', 'orders'), [
      {
        name: '_id',
        ordinal: 1,
        dataType: 'objectId',
        isNullable: false,
        defaultValue: null,
        isPrimaryKey: true,
        isForeignKey: false,
      },
    ]);
    objects(
      queryClient,
      'index',
      objectPage([
        {
          kind: 'index',
          name: '_id_',
          table: 'orders',
          method: 'regular',
          unique: true,
          primary: true,
          columns: ['_id'],
          predicate: null,
        },
      ]),
      'orders',
      MONGO,
      'loja',
      FLAT_SCHEMA,
    );

    const tree = rows(queryClient);
    const mongoAt = tree.findIndex(row => row.id === mongoDb);
    expect(below(tree, mongoDb).map(describeRow)).toEqual([
      'folder:Coleções',
      'table:orders',
      'folder:Campos',
      'column:_id',
      'folder:Índices',
      'index:_id_',
    ]);
    const index = tree[mongoAt + 6];
    expect(index.variant === 'node' && index.props.subLabel).toBe(
      '(_id) · regular · primário',
    );
  });
});
