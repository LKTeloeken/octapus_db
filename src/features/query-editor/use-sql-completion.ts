import { useEffect, useRef } from 'react';
import { useQueryClient } from '@tanstack/react-query';
import type { CompletionSource } from '@codemirror/autocomplete';
import type { Extension } from '@codemirror/state';
import { catalogComplete, catalogResolve } from '@/api/catalog';
import { listColumns } from '@/api/structure';
import {
  FLAT_SCHEMA,
  type CatalogNode,
  type CatalogNodeKind,
} from '@/api/types/catalog.types';
import type {
  ColumnInfo,
  DatabaseStructure,
  TableType,
} from '@/api/types/structure.types';
import {
  createSqlSchemaSource,
  sqlPrefetchListener,
} from '@/components/query-editor/query-editor/sql-completion/sql-schema-source';
import type { SqlCompletionPorts } from '@/components/query-editor/query-editor/sql-completion/sql-completion.types';
import { STRUCTURE_STALE_TIME_MS } from '@/providers/query-provider';
import { queryKeys } from '@/queries/keys';
import {
  catalogChildrenQuery,
  hasCatalog,
  isFlatCatalog,
} from '@/queries/use-catalog';
import { useServers } from '@/queries/use-servers';
import { useStructure } from '@/queries/use-structure';

export interface UseSqlCompletionParams {
  serverId: number;
  database: string;
  defaultSchema: string | null;
  enabled: boolean;
}

export interface SqlCompletion {
  source: CompletionSource;
  prefetch: Extension;
}

/** Relações por schema que a estrutura quente pede ao backend (teto do comando) */
const HOT_SCHEMA_LIMIT = 1_000;
/** Tabelas sem schema resolvidas por chamada (um JOIN grande não vira rajada) */
const MAX_RESOLVE = 8;

const TABLE_TYPE: Record<CatalogNodeKind, TableType> = {
  schema: 'table',
  table: 'table',
  partitioned: 'table',
  foreign: 'foreign',
  view: 'view',
  materializedView: 'materializedview',
};

/** Schemas quentes de um database e a estrutura montada a partir deles. */
interface HotStructure {
  key: string;
  schemas: Map<string, CatalogNode[]>;
  structure: DatabaseStructure;
}

/** A estrutura que o lang-sql compila, a partir dos schemas quentes */
const structureOf = (
  schemas: Map<string, CatalogNode[]>,
): DatabaseStructure => ({
  schemas: Array.from(schemas, ([name, nodes]) => ({
    name,
    tables: nodes.map(node => ({
      name: node.name,
      tableType: TABLE_TYPE[node.kind],
      sizeBytes: null,
    })),
  })),
  fetchedAt: Date.now(),
});

const emptyHot = (key: string): HotStructure => ({
  key,
  schemas: new Map(),
  structure: { schemas: [], fetchedAt: 0 },
});

/**
 * Autocomplete SQL do editor: estrutura do banco + colunas buscadas sob demanda.
 *
 * As colunas vivem na mesma key (`queryKeys.columns`) que a árvore de conexões preenche,
 * e esse cache é persistido em disco — expandir a tabela na árvore aquece o editor e
 * vice-versa, e depois de um restart não há request nenhum.
 *
 * Com catálogo no backend (Postgres) a estrutura não vem inteira: só a "quente" — o
 * schema da aba, o `public` e o que o statement cita, buscados pela porta `warm` —, e os
 * outros schemas entram por prefixo (`completeSchemas`). Compilar o namespace de 750 mil
 * tabelas travava a UI por mais de um segundo a cada coluna nova (BASELINE.md, H5).
 *
 * No SQLite (catálogo sem schema) a estrutura quente é o banco inteiro, sob o nome do
 * banco (`main`): ele vira o schema padrão, então `FROM t` e `FROM main.t` completam.
 * As colunas usam a mesma key da árvore (schema vazio).
 *
 * Num banco multi-tenant, as colunas de `tenant_42.orders` ainda não carregadas saem
 * na hora emprestadas de um tenant já aberto (`peekSimilarColumns`), marcadas como
 * prévia, e as de verdade chegam em segundo plano.
 *
 * A `CompletionSource` devolvida tem identidade fixa de propósito: ela é lida pelo
 * CodeMirror dentro de uma extensão, e trocar a função obrigaria a reconfigurar o editor
 * (o popup aberto se perderia). Por isso o alvo (`serverId`/`database`/estrutura) mora
 * num ref lido na hora da chamada — a aba pode trocar de banco sem remontar o painel.
 */
export function useSqlCompletion({
  serverId,
  database,
  defaultSchema,
  enabled,
}: UseSqlCompletionParams): SqlCompletion {
  const queryClient = useQueryClient();
  const { data: servers } = useServers();
  const dbType = servers?.find(server => server.id === serverId)?.dbType;
  const withCatalog = hasCatalog(dbType);
  const flat = isFlatCatalog(dbType);
  const { data: structure } = useStructure(serverId, database, {
    enabled: enabled && servers !== undefined && !withCatalog,
  });

  const target = useRef<{
    serverId: number;
    database: string;
    defaultSchema: string | null;
    structure: DatabaseStructure | undefined;
    withCatalog: boolean;
    flat: boolean;
  }>({ serverId, database, defaultSchema, structure, withCatalog, flat });

  target.current = {
    serverId,
    database,
    defaultSchema,
    structure,
    withCatalog,
    flat,
  };

  const hot = useRef<HotStructure>(emptyHot(''));
  const columnsVersion = useRef(0);
  const ports = useRef<SqlCompletionPorts | null>(null);

  if (!ports.current) {
    /** Estrutura quente do alvo atual (zera ao trocar de banco) */
    const hotFor = () => {
      const key = `${target.current.serverId}|${target.current.database}`;
      if (hot.current.key !== key) hot.current = emptyHot(key);
      return hot.current;
    };

    /** Sem schema, a key das colunas é a da árvore (schema vazio) */
    const columnSchema = (schema: string) =>
      target.current.flat ? FLAT_SCHEMA : schema;

    ports.current = {
      getStructure: () =>
        target.current.withCatalog
          ? hotFor().structure
          : target.current.structure,
      getDefaultSchema: () =>
        target.current.flat
          ? target.current.database
          : target.current.defaultSchema,

      peekColumns: (schema, table) =>
        queryClient.getQueryData<ColumnInfo[]>(
          queryKeys.columns(
            target.current.serverId,
            target.current.database,
            columnSchema(schema),
            table,
          ),
        ),

      ensureColumns: async (catalogSchema, table) => {
        const { serverId: id, database: db } = target.current;
        const schema = columnSchema(catalogSchema);

        try {
          // Devolve o cache quando existe e só busca quando falta; chamadas simultâneas
          // para a mesma tabela compartilham a mesma promise.
          return await queryClient.ensureQueryData({
            queryKey: queryKeys.columns(id, db, schema, table),
            queryFn: () => listColumns(id, db, schema, table),
            staleTime: STRUCTURE_STALE_TIME_MS,
            gcTime: STRUCTURE_STALE_TIME_MS,
          });
        } catch {
          // Tabela sem permissão ou removida: o resto do autocomplete segue funcionando.
          return undefined;
        }
      },

      getColumnsVersion: () => columnsVersion.current,

      // Multi-tenant: a mesma tabela num tenant já aberto empresta as colunas
      // enquanto as deste chegam (os tenants costumam ter o mesmo formato)
      peekSimilarColumns: (schema, table) => {
        const {
          serverId: id,
          database: db,
          withCatalog,
          flat,
        } = target.current;
        if (!withCatalog || flat) return undefined;
        for (const [key, columns] of queryClient.getQueriesData<ColumnInfo[]>({
          queryKey: queryKeys.columnsScope(id, db),
        })) {
          const [, , , donor, donorTable] = key as readonly unknown[];
          if (donorTable === table && donor !== schema && columns?.length) {
            return { columns, from: String(donor) };
          }
        }
        return undefined;
      },

      warm: async ({ schemas, tables }) => {
        if (!target.current.withCatalog) return;
        const {
          serverId: id,
          database: db,
          defaultSchema: tabSchema,
          flat,
        } = target.current;
        const state = hotFor();

        // Sem schema: o banco inteiro, uma vez, com o nome do banco
        if (flat) {
          try {
            const page = await queryClient.fetchQuery(
              catalogChildrenQuery(
                id,
                db,
                { kind: 'schema', schema: FLAT_SCHEMA },
                '',
                HOT_SCHEMA_LIMIT,
              ),
            );
            if (state.schemas.get(db) !== page.items && hot.current === state) {
              state.schemas.set(db, page.items);
              state.structure = structureOf(state.schemas);
            }
          } catch {
            // Catálogo indisponível: sem sugestões de tabela
          }
          return;
        }

        // O search_path padrão do Postgres termina no public
        const searchPath = Array.from(
          new Set([tabSchema ?? 'public', 'public']),
        );
        const wanted = new Set([...searchPath, ...schemas]);

        // Tabela sem schema que nenhum schema quente tem: o catálogo diz de onde é
        const known = new Set(
          Array.from(state.schemas.values()).flatMap(nodes =>
            nodes.map(node => node.name),
          ),
        );
        await Promise.all(
          tables
            .filter(table => !known.has(table))
            .slice(0, MAX_RESOLVE)
            .map(async table => {
              try {
                const resolution = await queryClient.fetchQuery({
                  queryKey: queryKeys.catalogResolve(id, db, table, searchPath),
                  queryFn: () => catalogResolve(id, db, table, searchPath),
                  staleTime: Infinity,
                });
                if (resolution.status === 'found')
                  wanted.add(resolution.schema);
              } catch {
                // Catálogo indisponível: segue com o que já é quente
              }
            }),
        );

        let changed = false;
        await Promise.all(
          Array.from(wanted).map(async schema => {
            try {
              // Do cache quando já veio (e não foi invalidado por um evento)
              const page = await queryClient.fetchQuery(
                catalogChildrenQuery(
                  id,
                  db,
                  { kind: 'schema', schema },
                  '',
                  HOT_SCHEMA_LIMIT,
                ),
              );
              if (state.schemas.get(schema) !== page.items) {
                state.schemas.set(schema, page.items);
                changed = true;
              }
            } catch {
              // Não é schema (alias, digitação pela metade): nada a aquecer
            }
          }),
        );

        if (changed && hot.current === state) {
          state.structure = structureOf(state.schemas);
        }
      },

      completeSchemas: async prefix => {
        if (!target.current.withCatalog || target.current.flat) return [];
        const { serverId: id, database: db } = target.current;
        try {
          const nodes = await queryClient.fetchQuery({
            queryKey: queryKeys.catalogComplete(id, db, null, prefix),
            queryFn: () => catalogComplete(id, db, null, prefix, 50),
            staleTime: 30 * 1000,
          });
          return nodes.map(node => node.name);
        } catch {
          return [];
        }
      },
    };
  }

  // Colunas podem chegar por fora (árvore de conexões). O contador invalida o namespace
  // já compilado pelo lang-sql, que não enxerga mutação depois de montado.
  useEffect(() => {
    return queryClient.getQueryCache().subscribe(event => {
      if (
        event.type === 'updated' &&
        event.action.type === 'success' &&
        event.query.queryKey[0] === 'columns'
      ) {
        columnsVersion.current += 1;
      }
    });
  }, [queryClient]);

  const completion = useRef<SqlCompletion | null>(null);

  if (!completion.current) {
    completion.current = {
      source: createSqlSchemaSource(ports.current),
      prefetch: sqlPrefetchListener(ports.current),
    };
  }

  return completion.current;
}
