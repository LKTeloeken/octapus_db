import { appendFileSync, mkdirSync } from 'node:fs';
import path from 'node:path';
import { CompletionContext } from '@codemirror/autocomplete';
import { PostgreSQL, sql } from '@codemirror/lang-sql';
import { EditorState } from '@codemirror/state';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { renderToString } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import type {
  CatalogNode,
  CatalogPage,
  CatalogSearchHit,
} from '@/api/types/catalog.types';
import type { Server } from '@/api/types/server.types';
import type { DatabaseStructure } from '@/api/types/structure.types';
import { createSqlSchemaSource } from '@/components/query-editor/query-editor/sql-completion/sql-schema-source';
import type { SqlCompletionPorts } from '@/components/query-editor/query-editor/sql-completion/sql-completion.types';
import {
  catalogHitToItem,
  flattenGroups,
  groupByServer,
} from '@/features/command-palette/palette-items';
import { useConnectionTree } from '@/features/connection-tree/use-connection-tree';
import { encodeNodeId } from '@/lib/node-ref';
import { queryKeys } from '@/queries/keys';
import { CATALOG_PAGE_SIZE } from '@/queries/use-catalog';
import { useTreeStore } from '@/stores/tree-store';
import { schemaName, tableName } from './structure-fixture';

/**
 * Fase 4 do refactor do catálogo: o custo na thread da UI dos caminhos novos
 * (fatias do catálogo), no mesmo cenário do tier L da baseline (5.000 schemas
 * × 150 tabelas). O backend é simulado pelo cache já populado — mede-se só o
 * que o front faz.
 *
 * Rodar: OCTAPUS_PERF=1 pnpm vitest run src/perf/catalog-front
 */

const TIER = 'L-catalog';
const SERVER_ID = 1;
const DATABASE = 'saas';
const SCHEMAS = 5_000;
const RELATIONS = 150;

const RESULTS_DIR = path.resolve(process.cwd(), 'perf/catalog/results');

function record(
  label: string,
  ms: number | null,
  extra: Record<string, unknown> = {},
) {
  const line = {
    suite: 'front',
    tier: TIER,
    net: 'n/a',
    case: label,
    ms: ms === null ? null : Math.round(ms * 100) / 100,
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

function median(samples: number, run: () => unknown): number {
  const times: number[] = [];
  for (let i = 0; i < samples; i++) {
    const start = performance.now();
    run();
    times.push(performance.now() - start);
  }
  times.sort((a, b) => a - b);
  return times[Math.floor(samples / 2)];
}

async function medianAsync(
  samples: number,
  run: () => Promise<unknown>,
): Promise<number> {
  const times: number[] = [];
  for (let i = 0; i < samples; i++) {
    const start = performance.now();
    await run();
    times.push(performance.now() - start);
  }
  times.sort((a, b) => a - b);
  return times[Math.floor(samples / 2)];
}

const SERVER: Server = {
  id: SERVER_ID,
  name: 'SaaS',
  dbType: 'postgres',
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

const schemaNode = (name: string): CatalogNode => ({
  name,
  kind: 'schema',
  childCount: RELATIONS,
  state: 'loaded',
  drift: null,
});

const relations = (): CatalogNode[] =>
  Array.from(
    { length: RELATIONS },
    (_, i): CatalogNode => ({
      name: tableName(i + 1),
      kind: 'table',
      childCount: null,
      state: null,
      drift: null,
    }),
  ).sort((a, b) => (a.name < b.name ? -1 : 1));

function treeClient(expandedSchemas: number) {
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false, staleTime: Infinity, gcTime: Infinity },
    },
  });
  queryClient.setQueryData(queryKeys.servers, [SERVER]);
  queryClient.setQueryData(queryKeys.databases(SERVER_ID), [
    { name: DATABASE, sizeBytes: null },
  ]);
  queryClient.setQueryData(queryKeys.capabilities(SERVER_ID), {
    hasSchemas: true,
    hasPrimaryKeys: true,
    supportsSql: true,
    supportsTransactions: true,
    supportsIndexes: true,
    browsable: true,
  });

  // O backend devolve uma janela de 500 dos 5.002 schemas
  const window: CatalogPage<CatalogNode> = {
    total: SCHEMAS + 2,
    offset: 0,
    items: Array.from({ length: CATALOG_PAGE_SIZE }, (_, i) =>
      schemaNode(schemaName(i + 1)),
    ),
  };
  queryClient.setQueryData(
    queryKeys.catalogChildren(
      SERVER_ID,
      DATABASE,
      { kind: 'schemas' },
      '',
      CATALOG_PAGE_SIZE,
    ),
    window,
  );
  const tables = relations();
  for (let i = 1; i <= expandedSchemas; i++) {
    queryClient.setQueryData(
      queryKeys.catalogChildren(
        SERVER_ID,
        DATABASE,
        { kind: 'schema', schema: schemaName(i) },
        '',
        CATALOG_PAGE_SIZE,
      ),
      { total: RELATIONS, offset: 0, items: tables },
    );
  }
  return queryClient;
}

function setExpanded(nodeIds: string[]) {
  const { expanded } = useTreeStore.getInitialState();
  expanded.clear();
  nodeIds.forEach(id => expanded.add(id));
}

function renderTree(queryClient: QueryClient) {
  let count = 0;
  function Probe() {
    count = useConnectionTree({ onEditServer: () => {} }).rows.length;
    return null;
  }
  renderToString(
    <QueryClientProvider client={queryClient}>
      <Probe />
    </QueryClientProvider>,
  );
  return count;
}

describe.skipIf(process.env.OCTAPUS_PERF !== '1')(
  'catálogo — front com fatias (Fase 4)',
  () => {
    it('payloads do IPC', () => {
      const window = JSON.stringify({
        total: SCHEMAS + 2,
        offset: 0,
        items: Array.from({ length: CATALOG_PAGE_SIZE }, (_, i) =>
          schemaNode(schemaName(i + 1)),
        ),
      });
      const hits = JSON.stringify(
        Array.from(
          { length: 50 },
          (_, i): CatalogSearchHit => ({
            serverId: SERVER_ID,
            database: DATABASE,
            name: tableName(i + 1),
            kind: 'table',
            score: 200 - i,
            schema: null,
            schemas: {
              total: 4_950,
              sample: [schemaName(1), schemaName(2), schemaName(3)],
            },
          }),
        ),
      );
      record(
        'IPC: JSON.parse de uma janela de 500 schemas',
        median(21, () => JSON.parse(window)),
        {
          kb: Math.round(window.length / 1024),
        },
      );
      record(
        'IPC: JSON.parse de 50 resultados de busca',
        median(21, () => JSON.parse(hits)),
        {
          kb: Math.round(hits.length / 1024),
        },
      );
    });

    it('árvore', () => {
      const serverNode = encodeNodeId({ serverId: SERVER_ID });
      const dbNode = encodeNodeId({ serverId: SERVER_ID, database: DATABASE });

      setExpanded([serverNode, dbNode]);
      const opened = treeClient(0);
      const rows = renderTree(opened);
      record(
        'árvore: database aberto (janela de 500 de 5.002)',
        median(11, () => renderTree(opened)),
        {
          rows,
        },
      );

      const expanded = Array.from({ length: 20 }, (_, i) =>
        encodeNodeId({
          serverId: SERVER_ID,
          database: DATABASE,
          schema: schemaName(i + 1),
        }),
      );
      setExpanded([serverNode, dbNode, ...expanded]);
      const twenty = treeClient(20);
      const rowsTwenty = renderTree(twenty);
      record(
        'árvore: 20 schemas abertos',
        median(11, () => renderTree(twenty)),
        {
          rows: rowsTwenty,
        },
      );
      expect(rowsTwenty).toBeGreaterThan(rows);
    });

    it('palette', () => {
      const hits: CatalogSearchHit[] = Array.from({ length: 50 }, (_, i) => ({
        serverId: SERVER_ID,
        database: DATABASE,
        name: tableName(i + 1),
        kind: 'table',
        score: 200 - i,
        schema: null,
        schemas: {
          total: 4_950,
          sample: [schemaName(1), schemaName(2), schemaName(3)],
        },
      }));
      record(
        'palette: 50 resultados do backend → itens',
        median(51, () =>
          flattenGroups(
            groupByServer(
              hits.map(hit => catalogHitToItem(hit, 'ord', SERVER)!),
            ),
          ),
        ),
        { items: hits.length },
      );
    });

    it('autocomplete', async () => {
      // Backend já em memória: o que pesa aqui é só o trabalho do front
      const tables = relations();
      const hot = new Map<string, CatalogNode[]>();
      let structure: DatabaseStructure = { schemas: [], fetchedAt: 0 };
      let version = 0;
      const allSchemas = Array.from({ length: SCHEMAS }, (_, i) =>
        schemaName(i + 1),
      );

      const ports: SqlCompletionPorts = {
        getStructure: () => structure,
        getDefaultSchema: () => null,
        peekColumns: () => undefined,
        ensureColumns: async () => undefined,
        getColumnsVersion: () => version,
        warm: async ({ schemas }) => {
          let changed = false;
          for (const schema of ['public', ...schemas]) {
            if (!hot.has(schema)) {
              hot.set(schema, tables);
              changed = true;
            }
          }
          if (changed) {
            structure = {
              schemas: Array.from(hot, ([name, nodes]) => ({
                name,
                tables: nodes.map(node => ({
                  name: node.name,
                  tableType: 'table' as const,
                  sizeBytes: null,
                })),
              })),
              fetchedAt: Date.now(),
            };
          }
        },
        completeSchemas: async prefix =>
          allSchemas.filter(schema => schema.startsWith(prefix)).slice(0, 50),
      };
      const source = createSqlSchemaSource(ports);
      const at = (doc: string) =>
        new CompletionContext(
          EditorState.create({
            doc,
            extensions: [sql({ dialect: PostgreSQL, upperCaseKeywords: true })],
          }),
          doc.length,
          true,
        );

      let start = performance.now();
      const first = await source(at('SELECT * FROM '));
      record(
        'autocomplete: 1ª sugestão (estrutura quente)',
        performance.now() - start,
        {
          options: first?.options.length ?? 0,
        },
      );

      record(
        'autocomplete: sugestão seguinte (memo)',
        await medianAsync(21, async () => source(at('SELECT * FROM '))),
      );

      version += 1;
      start = performance.now();
      await source(at('SELECT * FROM '));
      record(
        'autocomplete: depois de chegar coluna nova (recompila)',
        performance.now() - start,
      );

      start = performance.now();
      const schemaHits = await source(at('SELECT * FROM tenant_0004'));
      record(
        'autocomplete: schemas por prefixo (tenant_0004)',
        performance.now() - start,
        {
          options:
            schemaHits?.options.filter(option => option.type === 'namespace')
              .length ?? 0,
        },
      );

      start = performance.now();
      const dotted = await source(at('SELECT * FROM tenant_04242.'));
      record(
        'autocomplete: tenant_04242. (aquece o schema)',
        performance.now() - start,
        {
          options: dotted?.options.length ?? 0,
        },
      );
    });
  },
);
