import { appendFileSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import { CompletionContext } from '@codemirror/autocomplete';
import { PostgreSQL, sql } from '@codemirror/lang-sql';
import { EditorState } from '@codemirror/state';
import {
  QueryClient,
  QueryClientProvider,
  dehydrate,
} from '@tanstack/react-query';
import { renderToString } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import type { AdapterCapabilities } from '@/api/types/capabilities.types';
import type { Server } from '@/api/types/server.types';
import type {
  ColumnInfo,
  DatabaseStructure,
} from '@/api/types/structure.types';
import { createSqlSchemaSource } from '@/components/query-editor/query-editor/sql-completion/sql-schema-source';
import { resolveTable } from '@/components/query-editor/query-editor/sql-completion/sql-namespace';
import type { SqlCompletionPorts } from '@/components/query-editor/query-editor/sql-completion/sql-completion.types';
import { useTableIndex } from '@/features/command-palette/use-table-index';
import { useConnectionTree } from '@/features/connection-tree/use-connection-tree';
import { encodeNodeId } from '@/lib/node-ref';
import { queryKeys } from '@/queries/keys';
import { useTreeStore } from '@/stores/tree-store';
import {
  STRUCTURE_TIERS,
  makeStructure,
  schemaName,
  tableName,
  type StructureTier,
} from './structure-fixture';

/**
 * Baseline do front (Fase 0 do refactor do catálogo): quanto custa, na thread
 * da UI, cada consumidor do `DatabaseStructure` inteiro de hoje — o código real
 * dos hooks, renderizados uma vez com `renderToString` sobre um cache já
 * populado (nada vai ao backend).
 *
 * Rodar: OCTAPUS_PERF=1 pnpm vitest run  (tiers em OCTAPUS_PERF_FRONT_TIERS)
 */

const SERVER_ID = 1;
const DATABASE = 'tenants';
/** Tabelas cujas colunas o usuário já abriu (o autocomplete as espia todas) */
const COLUMNS_IN_CACHE = 50;

// Desde a Fase 4 o Postgres vai pelo catálogo do backend: a baseline mede o
// caminho antigo (estrutura inteira), que segue valendo para os bancos sem
// catálogo — daí um tipo sem catálogo aqui.
const SERVER: Server = {
  id: SERVER_ID,
  name: 'perf',
  dbType: 'sqlite',
  host: '127.0.0.1',
  port: 55432,
  username: 'postgres',
  defaultDatabase: DATABASE,
  sslEnabled: false,
  connectionUri: null,
  createdAt: 0,
  scopeDatabases: null,
  scopeSchemas: null,
};

const CAPABILITIES: AdapterCapabilities = {
  hasSchemas: true,
  hasPrimaryKeys: true,
  supportsSql: true,
  supportsTransactions: true,
  supportsIndexes: true,
  browsable: true,
};

const COLUMNS: ColumnInfo[] = [
  'id',
  'tenant_ref',
  'name',
  'payload',
  'created_at',
].map((name, index) => ({
  name,
  ordinal: index + 1,
  dataType: index === 0 ? 'bigint' : 'text',
  isNullable: index > 1,
  defaultValue: null,
  isPrimaryKey: index === 0,
  isForeignKey: false,
}));

const RESULTS_DIR = path.resolve(process.cwd(), 'perf/catalog/results');

function record(
  tier: StructureTier,
  label: string,
  ms: number | null,
  extra: Record<string, unknown>,
) {
  const line = {
    suite: 'front',
    tier: tier.label,
    net: 'n/a',
    case: label,
    ms: ms === null ? null : Math.round(ms * 10) / 10,
    extra,
    at: new Date().toISOString(),
  };
  mkdirSync(RESULTS_DIR, { recursive: true });
  appendFileSync(
    path.join(RESULTS_DIR, 'front.jsonl'),
    `${JSON.stringify(line)}\n`,
  );
  console.log(JSON.stringify(line));
}

function time<T>(run: () => T): [T, number] {
  const start = performance.now();
  const value = run();
  return [value, performance.now() - start];
}

async function timeAsync<T>(run: () => Promise<T>): Promise<[T, number]> {
  const start = performance.now();
  const value = await run();
  return [value, performance.now() - start];
}

function median(samples: number, run: () => unknown): number {
  const times: number[] = [];
  for (let i = 0; i < samples; i++) times.push(time(run)[1]);
  times.sort((a, b) => a - b);
  return times[Math.floor(samples / 2)];
}

const gc = (globalThis as { gc?: () => void }).gc;

function heapUsed(): number {
  gc?.();
  return process.memoryUsage().heapUsed;
}

/** Roda o hook dentro de um componente, uma renderização, e devolve o retorno. */
function renderHookOnce<T>(client: QueryClient, hook: () => T): T {
  let result: T | undefined;

  function Probe() {
    result = hook();
    return null;
  }

  renderToString(
    <QueryClientProvider client={client}>
      <Probe />
    </QueryClientProvider>,
  );

  return result as T;
}

/**
 * No `renderToString` o zustand lê o snapshot de servidor, que é o estado
 * *inicial* da store — `setState` não chega lá. Para o benchmark, o Set inicial
 * de nós expandidos é preenchido no lugar.
 */
function setExpandedForRender(nodeIds: string[]) {
  const { expanded } = useTreeStore.getInitialState();
  expanded.clear();
  for (const id of nodeIds) expanded.add(id);
}

function completionAt(doc: string, pos = doc.length): CompletionContext {
  const state = EditorState.create({
    doc,
    extensions: [sql({ dialect: PostgreSQL, upperCaseKeywords: true })],
  });
  return new CompletionContext(state, pos, true);
}

const tiers = (process.env.OCTAPUS_PERF_FRONT_TIERS ?? 'S,M,L,U')
  .split(',')
  .map(key => STRUCTURE_TIERS[key.trim()])
  .filter(Boolean);

describe('catálogo — baseline do front', () => {
  for (const tier of tiers) {
    it(`tier ${tier.label}`, async () => {
      const tables = tier.schemas * tier.tablesPerSchema;

      // ── IPC: o payload que o invoke entrega e o front precisa parsear ──
      const json = JSON.stringify(makeStructure(tier));
      const parseMs = median(3, () => JSON.parse(json));
      record(tier, 'IPC: JSON.parse do DatabaseStructure', parseMs, {
        tables,
        json_mb: Math.round((Buffer.byteLength(json) / 1e6) * 10) / 10,
      });

      const before = heapUsed();
      const structure: DatabaseStructure = JSON.parse(json);
      record(tier, 'heap: DatabaseStructure parseado', null, {
        heap_mb: Math.round(((heapUsed() - before) / 1e6) * 10) / 10,
      });

      const client = new QueryClient({
        defaultOptions: {
          queries: { retry: false, staleTime: Infinity, gcTime: Infinity },
        },
      });
      client.setQueryData(queryKeys.servers, [SERVER]);
      client.setQueryData(queryKeys.capabilities(SERVER_ID), CAPABILITIES);
      client.setQueryData(queryKeys.databases(SERVER_ID), [
        { name: DATABASE, sizeBytes: null },
      ]);
      client.setQueryData(queryKeys.structure(SERVER_ID, DATABASE), structure);
      for (let i = 1; i <= COLUMNS_IN_CACHE; i++) {
        client.setQueryData(
          queryKeys.columns(SERVER_ID, DATABASE, schemaName(i), tableName(1)),
          COLUMNS,
        );
      }

      // ── Persister: serializa o cache inteiro a cada mudança (throttle de 1 s) ──
      const persistMs = median(3, () =>
        JSON.stringify({
          timestamp: Date.now(),
          buster: '',
          clientState: dehydrate(client, {
            shouldDehydrateQuery: query => query.state.status === 'success',
          }),
        }),
      );
      record(
        tier,
        'persister: dehydrate + JSON.stringify (por gravação)',
        persistMs,
        {},
      );

      // ── Palette: achata todas as tabelas ao abrir ──
      const [entries, paletteMs] = time(() =>
        renderHookOnce(client, () => useTableIndex(true)),
      );
      expect(entries.length).toBe(tables);
      record(tier, 'palette: useTableIndex ao abrir', paletteMs, {
        entries: entries.length,
      });

      // ── Árvore: database aberto (lista os schemas) e com 20 schemas abertos ──
      const serverNode = encodeNodeId({ serverId: SERVER_ID });
      const databaseNode = encodeNodeId({
        serverId: SERVER_ID,
        database: DATABASE,
      });
      const onEditServer = () => {};

      setExpandedForRender([serverNode, databaseNode]);
      const treeMs = median(3, () =>
        renderHookOnce(client, () => useConnectionTree({ onEditServer })),
      );
      const tree = renderHookOnce(client, () =>
        useConnectionTree({ onEditServer }),
      );
      record(tier, 'árvore: render com o database aberto', treeMs, {
        rows: tree.rows.length,
      });

      const expandedSchemas = Array.from({ length: 20 }, (_, i) =>
        encodeNodeId({
          serverId: SERVER_ID,
          database: DATABASE,
          schema: schemaName(i + 1),
        }),
      );
      setExpandedForRender([serverNode, databaseNode, ...expandedSchemas]);
      const treeExpandedMs = median(3, () =>
        renderHookOnce(client, () => useConnectionTree({ onEditServer })),
      );
      record(tier, 'árvore: render com 20 schemas abertos', treeExpandedMs, {
        rows: renderHookOnce(client, () => useConnectionTree({ onEditServer }))
          .rows.length,
      });

      // ── Autocomplete: as mesmas portas do useSqlCompletion ──
      let columnsVersion = 0;
      const ports: SqlCompletionPorts = {
        getStructure: () => structure,
        getDefaultSchema: () => null,
        peekColumns: (schema, table) =>
          client.getQueryData<ColumnInfo[]>(
            queryKeys.columns(SERVER_ID, DATABASE, schema, table),
          ),
        ensureColumns: async () => undefined,
        getColumnsVersion: () => columnsVersion,
      };
      const source = createSqlSchemaSource(ports);

      const [first, firstMs] = await timeAsync(async () =>
        source(completionAt('SELECT * FROM ')),
      );
      record(tier, 'autocomplete: 1ª sugestão (compila o namespace)', firstMs, {
        options: first?.options.length ?? 0,
      });

      const [, memoMs] = await timeAsync(async () =>
        source(completionAt('SELECT * FROM ')),
      );
      record(tier, 'autocomplete: sugestão seguinte (memo)', memoMs, {});

      columnsVersion += 1;
      const [, rebuildMs] = await timeAsync(async () =>
        source(completionAt('SELECT * FROM ')),
      );
      record(
        tier,
        'autocomplete: depois de chegar coluna nova (recompila)',
        rebuildMs,
        {},
      );

      const [, unqualifiedMs] = await timeAsync(async () =>
        source(completionAt('SELECT  FROM orders', 'SELECT '.length)),
      );
      record(
        tier,
        'autocomplete: SELECT | FROM orders (tabela sem schema)',
        unqualifiedMs,
        {},
      );

      const resolveMs = median(5, () =>
        resolveTable(structure, 'orders_links'),
      );
      record(tier, 'autocomplete: resolveTable sem schema', resolveMs, {});
    });
  }
});
