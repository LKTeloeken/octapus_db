import { Add01Icon, Edit01Icon, RefreshIcon } from '@hugeicons/core-free-icons';
import { useQueries, type UseQueryResult } from '@tanstack/react-query';
import { useMemo } from 'react';
import { getCapabilities } from '@/api/connection';
import { listColumns, listDatabases, listSchemasWithTables } from '@/api/structure';
import type { AdapterCapabilities } from '@/api/types/capabilities.types';
import type {
  ColumnInfo,
  DatabaseInfo,
  DatabaseStructure,
} from '@/api/types/structure.types';
import { useServers } from '@/queries/use-servers';
import { useRefreshStructure } from '@/queries/use-refresh-structure';
import { queryKeys } from '@/queries/keys';
import { STRUCTURE_STALE_TIME_MS } from '@/providers/query-provider';
import { useFocusStore } from '@/stores/focus-store';
import { useTabsStore } from '@/stores/tabs-store';
import { useTreeStore } from '@/stores/tree-store';
import { DEFAULT_DATABASES } from '@/lib/db-defaults';
import { decodeNodeId, encodeNodeId, nodeKind } from '@/lib/node-ref';
import type { ConnectionTreeProps, FlatRow } from './connection-tree.types';

const structureQueryOptions = {
  staleTime: STRUCTURE_STALE_TIME_MS,
  gcTime: STRUCTURE_STALE_TIME_MS,
} as const;

const columnsKey = (
  serverId: number,
  database: string,
  schema: string | null,
  table: string,
) => `${serverId}|${database}|${schema ?? ''}|${table}`;

/**
 * Flattens the expanded tree into a single virtualizable list. Each level's
 * data is fetched lazily via `useQueries`, driven entirely by the expanded
 * node-id set (decoded back into typed refs) — there is no per-node component
 * holding state, which is what makes the tree safe to virtualize.
 */
export const useConnectionTree = ({ onEditServer }: ConnectionTreeProps) => {
  const serversQuery = useServers();
  const expanded = useTreeStore(state => state.expanded);
  const toggleNode = useTreeStore(state => state.toggleNode);
  const openQueryTab = useTabsStore(state => state.openQueryTab);
  const openBrowseTab = useTabsStore(state => state.openBrowseTab);
  const requestFocus = useFocusStore(state => state.requestFocus);
  const { refreshServer, refreshDatabase, refreshSchema, refreshTable } =
    useRefreshStructure();

  const servers = serversQuery.data ?? [];
  const serverIds = useMemo(
    () => new Set(servers.map(server => server.id)),
    [servers],
  );

  // Decode the expansion set into the entities whose children must be fetched.
  const expandedRefs = useMemo(
    () => Array.from(expanded).map(decodeNodeId),
    [expanded],
  );
  const expandedServerIds = useMemo(
    () =>
      expandedRefs
        .filter(ref => nodeKind(ref) === 'server' && serverIds.has(ref.serverId))
        .map(ref => ref.serverId),
    [expandedRefs, serverIds],
  );
  const expandedDbRefs = useMemo(
    () =>
      expandedRefs.filter(
        ref => nodeKind(ref) === 'database' && serverIds.has(ref.serverId),
      ),
    [expandedRefs, serverIds],
  );
  const expandedTableRefs = useMemo(
    () =>
      expandedRefs.filter(
        ref => nodeKind(ref) === 'table' && serverIds.has(ref.serverId),
      ),
    [expandedRefs, serverIds],
  );

  const databaseQueries = useQueries({
    queries: expandedServerIds.map(serverId => ({
      queryKey: queryKeys.databases(serverId),
      queryFn: () => listDatabases(serverId),
      ...structureQueryOptions,
    })),
  });

  const capabilityQueries = useQueries({
    queries: expandedServerIds.map(serverId => ({
      queryKey: queryKeys.capabilities(serverId),
      queryFn: () => getCapabilities(serverId),
      ...structureQueryOptions,
    })),
  });

  const structureQueries = useQueries({
    queries: expandedDbRefs.map(ref => ({
      queryKey: queryKeys.structure(ref.serverId, ref.database!),
      queryFn: () => listSchemasWithTables(ref.serverId, ref.database!),
      ...structureQueryOptions,
    })),
  });

  const columnQueries = useQueries({
    queries: expandedTableRefs.map(ref => ({
      queryKey: queryKeys.columns(
        ref.serverId,
        ref.database!,
        ref.schema ?? '',
        ref.table!,
      ),
      queryFn: () =>
        listColumns(ref.serverId, ref.database!, ref.schema ?? '', ref.table!),
      ...structureQueryOptions,
    })),
  });

  // Zip query results back to their entities (results keep input order).
  const databasesByServer = new Map<number, UseQueryResult<DatabaseInfo[]>>();
  expandedServerIds.forEach((id, i) => databasesByServer.set(id, databaseQueries[i]));

  const capabilitiesByServer = new Map<
    number,
    UseQueryResult<AdapterCapabilities>
  >();
  expandedServerIds.forEach((id, i) =>
    capabilitiesByServer.set(id, capabilityQueries[i]),
  );

  const structureByKey = new Map<string, UseQueryResult<DatabaseStructure>>();
  expandedDbRefs.forEach((ref, i) =>
    structureByKey.set(`${ref.serverId}|${ref.database}`, structureQueries[i]),
  );

  const columnsByKey = new Map<string, UseQueryResult<ColumnInfo[]>>();
  expandedTableRefs.forEach((ref, i) =>
    columnsByKey.set(
      columnsKey(ref.serverId, ref.database!, ref.schema ?? null, ref.table!),
      columnQueries[i],
    ),
  );

  const rows: FlatRow[] = [];

  // Abrir uma tabela entrega o teclado para a grade — o pedido fica pendente
  // até o ResultsTable montar, já que a aba nasce antes dos dados chegarem.
  const openTable = (
    serverId: number,
    database: string,
    schema: string | null,
    table: string,
  ) => {
    openBrowseTab({ serverId, database, schema, table });
    requestFocus('grid');
  };

  const pushTable = (
    serverId: number,
    database: string,
    schema: string | null,
    table: string,
    level: number,
  ) => {
    const tableNodeId = encodeNodeId({
      serverId,
      database,
      schema: schema ?? undefined,
      table,
    });
    const isExpanded = expanded.has(tableNodeId);
    const colQuery = columnsByKey.get(
      columnsKey(serverId, database, schema, table),
    );

    rows.push({
      variant: 'node',
      id: tableNodeId,
      props: {
        level,
        kind: 'table',
        name: table,
        hasChildren: true,
        isExpanded,
        isLoading: isExpanded && (colQuery?.isFetching ?? false),
        onClick: () => {
          toggleNode(tableNodeId);
          openTable(serverId, database, schema, table);
        },
        actions: [
          {
            label: 'Atualizar',
            icon: RefreshIcon,
            onSelect: () =>
              void refreshTable(serverId, database, schema, table),
          },
        ],
      },
      // Pelo teclado o Enter só abre a tabela: expandir/colapsar as colunas já
      // é papel das setas ←/→.
      onEnter: () => openTable(serverId, database, schema, table),
    });

    if (!isExpanded) return;

    if (colQuery?.isError) {
      rows.push({
        variant: 'error',
        id: `${tableNodeId}|err`,
        level: level + 1,
        message: colQuery.error.message,
        onRetry: () => void colQuery.refetch(),
      });
      return;
    }

    for (const column of colQuery?.data ?? []) {
      rows.push({
        variant: 'node',
        id: `${tableNodeId}|c=${column.name}`,
        props: {
          level: level + 1,
          kind: 'column',
          name: column.name,
          subLabel: `${column.dataType}${column.isPrimaryKey ? ' (PK)' : ''}`,
          hasChildren: false,
        },
      });
    }
  };

  for (const server of servers) {
    const serverNodeId = encodeNodeId({ serverId: server.id });
    const isExpanded = expanded.has(serverNodeId);
    const dbQuery = databasesByServer.get(server.id);

    rows.push({
      variant: 'node',
      id: serverNodeId,
      props: {
        level: 0,
        kind: 'server',
        name: server.name,
        hasChildren: true,
        isExpanded,
        isLoading: isExpanded && (dbQuery?.isFetching ?? false),
        isHighlighted: dbQuery?.isSuccess ?? false,
        onClick: () => toggleNode(serverNodeId),
        actions: [
          {
            label: 'Atualizar',
            icon: RefreshIcon,
            onSelect: () => void refreshServer(server.id),
          },
          {
            label: 'Editar',
            icon: Edit01Icon,
            onSelect: () => onEditServer(server),
          },
          {
            label: 'Nova aba',
            icon: Add01Icon,
            onSelect: () =>
              openQueryTab({
                serverId: server.id,
                database:
                  server.defaultDatabase ?? DEFAULT_DATABASES[server.dbType],
              }),
          },
        ],
      },
    });

    if (!isExpanded) continue;

    if (dbQuery?.isError) {
      rows.push({
        variant: 'error',
        id: `${serverNodeId}|err`,
        level: 1,
        message: dbQuery.error.message,
        onRetry: () => void dbQuery.refetch(),
      });
      continue;
    }

    // Mongo/Redis have no schema level — render tables directly (BACKEND.md §6.3)
    const hasSchemas =
      capabilitiesByServer.get(server.id)?.data?.hasSchemas ?? true;

    for (const db of dbQuery?.data ?? []) {
      const dbNodeId = encodeNodeId({ serverId: server.id, database: db.name });
      const isDbExpanded = expanded.has(dbNodeId);
      const structureQuery = structureByKey.get(`${server.id}|${db.name}`);

      rows.push({
        variant: 'node',
        id: dbNodeId,
        props: {
          level: 1,
          kind: 'database',
          name: db.name,
          hasChildren: true,
          isExpanded: isDbExpanded,
          isLoading: isDbExpanded && (structureQuery?.isFetching ?? false),
          onClick: () => toggleNode(dbNodeId),
          actions: [
            {
              label: 'Atualizar',
              icon: RefreshIcon,
              onSelect: () => void refreshDatabase(server.id, db.name),
            },
            {
              label: 'Nova aba',
              icon: Add01Icon,
              onSelect: () =>
                openQueryTab({ serverId: server.id, database: db.name }),
            },
          ],
        },
      });

      if (!isDbExpanded) continue;

      if (structureQuery?.isError) {
        rows.push({
          variant: 'error',
          id: `${dbNodeId}|err`,
          level: 2,
          message: structureQuery.error.message,
          onRetry: () => void structureQuery.refetch(),
        });
        continue;
      }

      const structure = structureQuery?.data;
      if (!structure) continue;

      if (hasSchemas) {
        for (const schema of structure.schemas) {
          const schemaNodeId = encodeNodeId({
            serverId: server.id,
            database: db.name,
            schema: schema.name,
          });
          const isSchemaExpanded = expanded.has(schemaNodeId);

          rows.push({
            variant: 'node',
            id: schemaNodeId,
            props: {
              level: 2,
              kind: 'schema',
              name: schema.name,
              subLabel: String(schema.tables.length),
              hasChildren: schema.tables.length > 0,
              isExpanded: isSchemaExpanded,
              onClick: () => toggleNode(schemaNodeId),
              actions: [
                {
                  label: 'Atualizar',
                  icon: RefreshIcon,
                  onSelect: () =>
                    void refreshSchema(server.id, db.name, schema.name),
                },
              ],
            },
          });

          if (!isSchemaExpanded) continue;

          for (const table of schema.tables) {
            pushTable(server.id, db.name, schema.name, table.name, 3);
          }
        }
      } else {
        for (const table of structure.schemas.flatMap(s => s.tables)) {
          pushTable(server.id, db.name, null, table.name, 2);
        }
      }
    }
  }

  return {
    rows,
    isLoading: serversQuery.isLoading,
    isError: serversQuery.isError,
    error: serversQuery.error,
    retry: () => void serversQuery.refetch(),
    isEmpty: !serversQuery.data?.length,
  };
};
