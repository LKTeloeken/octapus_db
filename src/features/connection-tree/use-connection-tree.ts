import {
  Add01Icon,
  Edit01Icon,
  LayoutGridIcon,
  ListViewIcon,
  RefreshIcon,
  Stethoscope02Icon,
} from '@hugeicons/core-free-icons';
import { useQueries, type UseQueryResult } from '@tanstack/react-query';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { useMemo } from 'react';
import toast from 'react-hot-toast';
import { getCapabilities } from '@/api/connection';
import { listColumns, listDatabases, listSchemasWithTables } from '@/api/structure';
import type { AdapterCapabilities } from '@/api/types/capabilities.types';
import {
  FLAT_SCHEMA,
  type CatalogNode,
  type CatalogNodeKind,
  type CatalogPage,
  type CatalogStatus,
  type ShapeGroup,
} from '@/api/types/catalog.types';
import type { DatabaseType } from '@/api/types/server.types';
import type {
  ColumnInfo,
  DatabaseInfo,
  DatabaseStructure,
} from '@/api/types/structure.types';
import { useServers } from '@/queries/use-servers';
import { useRefreshStructure } from '@/queries/use-refresh-structure';
import {
  CATALOG_PAGE_SIZE,
  catalogChildrenQuery,
  catalogShapesQuery,
  catalogStatusQuery,
  hasCatalog,
  isFlatCatalog,
  useCatalogDiagnostics,
  useCatalogRelationSize,
} from '@/queries/use-catalog';
import { useCatalogEvents } from '@/queries/use-catalog-events';
import { queryKeys } from '@/queries/keys';
import { STRUCTURE_STALE_TIME_MS } from '@/providers/query-provider';
import { useFocusStore } from '@/stores/focus-store';
import { useTabsStore } from '@/stores/tabs-store';
import { useTreeStore } from '@/stores/tree-store';
import { formatCatalogDiagnostics } from '@/lib/catalog-diagnostics';
import { DEFAULT_DATABASES } from '@/lib/db-defaults';
import { formatCount } from '@/lib/format-count';
import { decodeNodeId, encodeNodeId, nodeKind, type NodeRef } from '@/lib/node-ref';
import type { ConnectionTreeProps, FlatRow } from './connection-tree.types';
import { driftBadge, shapeGroupRow } from './shape-labels';

const structureQueryOptions = {
  staleTime: STRUCTURE_STALE_TIME_MS,
  gcTime: STRUCTURE_STALE_TIME_MS,
} as const;

/** A partir de quantos filhos um nó ganha o campo de filtro */
const FILTER_FROM = 50;

const columnsKey = (
  serverId: number,
  database: string,
  schema: string | null,
  table: string,
) => `${serverId}|${database}|${schema ?? ''}|${table}`;

const dbKey = (ref: NodeRef) => `${ref.serverId}|${ref.database}`;
const schemaKey = (ref: NodeRef) => `${ref.serverId}|${ref.database}|${ref.schema}`;

/**
 * Flattens the expanded tree into a single virtualizable list. Each level's
 * data is fetched lazily via `useQueries`, driven entirely by the expanded
 * node-id set (decoded back into typed refs) — there is no per-node component
 * holding state, which is what makes the tree safe to virtualize.
 *
 * Bancos com catálogo no backend pedem fatias: os schemas de um database e as
 * relações de um schema vêm em janelas de 500, com filtro quando a lista é
 * grande — 5.000 schemas de tenant não viram 5.000 linhas de uma vez. Mongo e
 * SQLite não têm schema: as coleções/tabelas vêm direto sob o database, do
 * schema sem nome (`FLAT_SCHEMA`). O Redis segue com a estrutura inteira
 * (`list_schemas_with_tables`).
 *
 * A lista de databases de um servidor (um database por tenant) ganha o mesmo
 * filtro e a mesma janela, aplicados aqui: só os nomes vêm do backend.
 *
 * Um database multi-tenant pode ser visto agrupado por formato: o molde dos
 * tenants, as variações (tenant com migração pendente) e o resto. Cada schema
 * fora do molde leva um aviso com quantas tabelas faltam ou sobram.
 */
export const useConnectionTree = ({ onEditServer }: ConnectionTreeProps) => {
  useCatalogEvents();

  const serversQuery = useServers();
  const expanded = useTreeStore(state => state.expanded);
  const toggleNode = useTreeStore(state => state.toggleNode);
  const filters = useTreeStore(state => state.filters);
  const limits = useTreeStore(state => state.limits);
  const setFilter = useTreeStore(state => state.setFilter);
  const showMore = useTreeStore(state => state.showMore);
  const shapeGrouped = useTreeStore(state => state.shapeGrouped);
  const toggleShapeGrouping = useTreeStore(state => state.toggleShapeGrouping);
  const diagnostics = useCatalogDiagnostics();
  const openQueryTab = useTabsStore(state => state.openQueryTab);
  const openBrowseTab = useTabsStore(state => state.openBrowseTab);
  // A aba de tabela tem o mesmo id do nó da árvore: basta comparar.
  const activeTabId = useTabsStore(state => state.activeTabId);
  const requestFocus = useFocusStore(state => state.requestFocus);
  const { refreshServer, refreshDatabase, refreshSchema, refreshTable } =
    useRefreshStructure();

  const servers = serversQuery.data ?? [];
  const serverIds = useMemo(
    () => new Set(servers.map(server => server.id)),
    [servers],
  );
  const catalogServerIds = useMemo(
    () =>
      new Set(
        servers.filter(server => hasCatalog(server.dbType)).map(server => server.id),
      ),
    [servers],
  );
  const flatServerIds = useMemo(
    () =>
      new Set(
        servers.filter(server => isFlatCatalog(server.dbType)).map(server => server.id),
      ),
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
  const legacyDbRefs = useMemo(
    () => expandedDbRefs.filter(ref => !catalogServerIds.has(ref.serverId)),
    [expandedDbRefs, catalogServerIds],
  );
  const catalogDbRefs = useMemo(
    () =>
      expandedDbRefs.filter(
        ref => catalogServerIds.has(ref.serverId) && !flatServerIds.has(ref.serverId),
      ),
    [expandedDbRefs, catalogServerIds, flatServerIds],
  );
  const flatDbRefs = useMemo(
    () => expandedDbRefs.filter(ref => flatServerIds.has(ref.serverId)),
    [expandedDbRefs, flatServerIds],
  );
  // Só schemas visíveis: um schema continua no conjunto expandido quando o
  // database fecha, e não vale ir ao backend por ele
  const catalogSchemaRefs = useMemo(
    () =>
      expandedRefs.filter(
        ref =>
          nodeKind(ref) === 'schema' &&
          catalogServerIds.has(ref.serverId) &&
          expanded.has(encodeNodeId({ serverId: ref.serverId, database: ref.database })),
      ),
    [expandedRefs, catalogServerIds, expanded],
  );
  const expandedTableRefs = useMemo(
    () =>
      expandedRefs.filter(
        ref => nodeKind(ref) === 'table' && serverIds.has(ref.serverId),
      ),
    [expandedRefs, serverIds],
  );

  // Tamanho só da tabela aberta (de um banco com catálogo), sob demanda
  const openRef = useMemo(() => {
    if (!activeTabId) return null;
    const ref = decodeNodeId(activeTabId);
    return nodeKind(ref) === 'table' && catalogServerIds.has(ref.serverId)
      ? {
          serverId: ref.serverId,
          database: ref.database!,
          schema: ref.schema ?? FLAT_SCHEMA,
          table: ref.table!,
        }
      : null;
  }, [activeTabId, catalogServerIds]);
  const openSize = useCatalogRelationSize(openRef);

  const windowOf = (nodeId: string) => ({
    filter: filters.get(nodeId) ?? '',
    limit: limits.get(nodeId) ?? CATALOG_PAGE_SIZE,
  });

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
    queries: legacyDbRefs.map(ref => ({
      queryKey: queryKeys.structure(ref.serverId, ref.database!),
      queryFn: () => listSchemasWithTables(ref.serverId, ref.database!),
      ...structureQueryOptions,
    })),
  });

  const catalogStatusRefs = useMemo(
    () => [...catalogDbRefs, ...flatDbRefs],
    [catalogDbRefs, flatDbRefs],
  );
  const catalogStatusQueries = useQueries({
    queries: catalogStatusRefs.map(ref => catalogStatusQuery(ref.serverId, ref.database!)),
  });

  // Agrupados por formato: os grupos primeiro; a lista simples só sem eles
  const groupedDbRefs = catalogDbRefs.filter(ref =>
    shapeGrouped.has(encodeNodeId(ref)),
  );
  const shapeQueries = useQueries({
    queries: groupedDbRefs.map(ref =>
      catalogShapesQuery(ref.serverId, ref.database!),
    ),
  });
  const shapesByKey = new Map<string, UseQueryResult<ShapeGroup[]>>();
  groupedDbRefs.forEach((ref, i) => shapesByKey.set(dbKey(ref), shapeQueries[i]));
  /** Sem formato repetido não há grupos: a árvore fica na lista simples */
  const listDbRefs = catalogDbRefs.filter(ref => {
    const shapes = shapesByKey.get(dbKey(ref));
    return !shapes || shapes.data?.length === 0;
  });

  const expandedShapeRefs = expandedRefs.filter(ref => {
    if (nodeKind(ref) !== 'shape') return false;
    const groups = shapesByKey.get(dbKey(ref))?.data;
    return (
      expanded.has(encodeNodeId({ serverId: ref.serverId, database: ref.database })) &&
      !!groups?.some(group => group.key === ref.shape)
    );
  });
  const shapeSchemaQueries = useQueries({
    queries: expandedShapeRefs.map(ref => {
      const { filter, limit } = windowOf(encodeNodeId(ref));
      return catalogChildrenQuery(
        ref.serverId,
        ref.database!,
        { kind: 'shape', key: ref.shape! },
        filter,
        limit,
      );
    }),
  });
  const shapeSchemasByKey = new Map<
    string,
    UseQueryResult<CatalogPage<CatalogNode>>
  >();
  expandedShapeRefs.forEach((ref, i) =>
    shapeSchemasByKey.set(`${dbKey(ref)}|${ref.shape}`, shapeSchemaQueries[i]),
  );

  const schemaListQueries = useQueries({
    queries: listDbRefs.map(ref => {
      const { filter, limit } = windowOf(encodeNodeId(ref));
      return catalogChildrenQuery(
        ref.serverId,
        ref.database!,
        { kind: 'schemas' },
        filter,
        limit,
      );
    }),
  });

  // Sem schema: as relações vêm direto sob o database
  const flatListQueries = useQueries({
    queries: flatDbRefs.map(ref => {
      const { filter, limit } = windowOf(encodeNodeId(ref));
      return catalogChildrenQuery(
        ref.serverId,
        ref.database!,
        { kind: 'schema', schema: FLAT_SCHEMA },
        filter,
        limit,
      );
    }),
  });

  const relationListQueries = useQueries({
    queries: catalogSchemaRefs.map(ref => {
      const { filter, limit } = windowOf(encodeNodeId(ref));
      return catalogChildrenQuery(
        ref.serverId,
        ref.database!,
        { kind: 'schema', schema: ref.schema! },
        filter,
        limit,
      );
    }),
  });

  // Tipo de cada relação já listada: uma particionada expande para as
  // partições, as demais para as colunas
  const relationKinds = new Map<string, CatalogNodeKind>();
  catalogSchemaRefs.forEach((ref, i) => {
    for (const node of relationListQueries[i]?.data?.items ?? []) {
      relationKinds.set(
        columnsKey(ref.serverId, ref.database!, ref.schema!, node.name),
        node.kind,
      );
    }
  });
  const isPartitioned = (ref: NodeRef) =>
    relationKinds.get(
      columnsKey(ref.serverId, ref.database!, ref.schema ?? null, ref.table!),
    ) === 'partitioned';

  const partitionedRefs = expandedTableRefs.filter(isPartitioned);
  const columnRefs = expandedTableRefs.filter(ref => !isPartitioned(ref));

  const partitionQueries = useQueries({
    queries: partitionedRefs.map(ref => {
      const { filter, limit } = windowOf(encodeNodeId(ref));
      return catalogChildrenQuery(
        ref.serverId,
        ref.database!,
        { kind: 'partitions', schema: ref.schema!, table: ref.table! },
        filter,
        limit,
      );
    }),
  });

  const columnQueries = useQueries({
    queries: columnRefs.map(ref => ({
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
  legacyDbRefs.forEach((ref, i) => structureByKey.set(dbKey(ref), structureQueries[i]));

  const statusByKey = new Map<string, UseQueryResult<CatalogStatus>>();
  catalogStatusRefs.forEach((ref, i) => statusByKey.set(dbKey(ref), catalogStatusQueries[i]));

  // Filhos do database com catálogo: schemas ou, sem schema, as relações
  const dbChildrenByKey = new Map<string, UseQueryResult<CatalogPage<CatalogNode>>>();
  listDbRefs.forEach((ref, i) => dbChildrenByKey.set(dbKey(ref), schemaListQueries[i]));
  flatDbRefs.forEach((ref, i) => dbChildrenByKey.set(dbKey(ref), flatListQueries[i]));

  const relationListByKey = new Map<string, UseQueryResult<CatalogPage<CatalogNode>>>();
  catalogSchemaRefs.forEach((ref, i) =>
    relationListByKey.set(schemaKey(ref), relationListQueries[i]),
  );

  const partitionsByKey = new Map<string, UseQueryResult<CatalogPage<CatalogNode>>>();
  partitionedRefs.forEach((ref, i) =>
    partitionsByKey.set(
      columnsKey(ref.serverId, ref.database!, ref.schema ?? null, ref.table!),
      partitionQueries[i],
    ),
  );

  const columnsByKey = new Map<string, UseQueryResult<ColumnInfo[]>>();
  columnRefs.forEach((ref, i) =>
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

  /**
   * Filtro (lista grande) antes dos filhos de `nodeId`, e "carregar mais"
   * depois deles quando a janela não cobre o total.
   */
  const pushPage = <T,>(
    nodeId: string,
    level: number,
    page: CatalogPage<T>,
    isFetching: boolean,
    placeholder: string,
    pushChild: (item: T) => void,
  ) => {
    const filter = filters.get(nodeId) ?? '';

    if (filter || page.total > FILTER_FROM) {
      rows.push({
        variant: 'filter',
        id: `${nodeId}|filter`,
        level,
        nodeId,
        value: filter,
        total: page.total,
        placeholder,
        onChange: value => setFilter(nodeId, value),
      });
    }

    for (const item of page.items) pushChild(item);

    const shown = page.offset + page.items.length;
    if (shown < page.total) {
      const limit = limits.get(nodeId) ?? CATALOG_PAGE_SIZE;
      rows.push({
        variant: 'more',
        id: `${nodeId}|more`,
        level,
        remaining: page.total - shown,
        isLoading: isFetching,
        onMore: () => showMore(nodeId, limit, CATALOG_PAGE_SIZE),
      });
    }
  };

  /** Uma janela vinda do catálogo (o filtro e o limite já foram no pedido) */
  const pushWindowed = (
    nodeId: string,
    level: number,
    query: UseQueryResult<CatalogPage<CatalogNode>> | undefined,
    placeholder: string,
    pushChild: (node: CatalogNode) => void,
  ) => {
    if (query?.isError) {
      rows.push({
        variant: 'error',
        id: `${nodeId}|err`,
        level,
        message: query.error.message,
        onRetry: () => void query.refetch(),
      });
      return;
    }

    const page = query?.data;
    if (!page) return;
    pushPage(nodeId, level, page, query.isFetching, placeholder, pushChild);
  };

  /** A mesma janela sobre uma lista que já está inteira no front */
  const localPage = <T,>(nodeId: string, items: T[], nameOf: (item: T) => string) => {
    const needle = (filters.get(nodeId) ?? '').trim().toLowerCase();
    const matching = needle
      ? items.filter(item => nameOf(item).toLowerCase().includes(needle))
      : items;
    const limit = limits.get(nodeId) ?? CATALOG_PAGE_SIZE;
    return { total: matching.length, offset: 0, items: matching.slice(0, limit) };
  };

  const pushTable = (
    serverId: number,
    database: string,
    schema: string | null,
    table: string,
    sizeBytes: number | null,
    level: number,
    relationKind?: CatalogNodeKind,
  ) => {
    const tableNodeId = encodeNodeId({
      serverId,
      database,
      schema: schema ?? undefined,
      table,
    });
    const isExpanded = expanded.has(tableNodeId);
    const key = columnsKey(serverId, database, schema, table);
    const partitioned = relationKind === 'partitioned';
    const childQuery = partitioned ? partitionsByKey.get(key) : columnsByKey.get(key);

    rows.push({
      variant: 'node',
      id: tableNodeId,
      props: {
        level,
        kind: 'table',
        relationKind,
        name: table,
        sizeBytes,
        hasChildren: true,
        isExpanded,
        isLoading: isExpanded && (childQuery?.isFetching ?? false),
        isOpen: activeTabId === tableNodeId,
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
          {
            label: 'Nova aba',
            icon: Add01Icon,
            onSelect: () =>
              openQueryTab({
                serverId,
                database,
                schema,
                title: table,
              }),
          },
        ],
      },
      // Pelo teclado o Enter só abre a tabela: expandir/colapsar as colunas já
      // é papel das setas ←/→.
      onEnter: () => openTable(serverId, database, schema, table),
    });

    if (!isExpanded) return;

    if (partitioned) {
      pushWindowed(
        tableNodeId,
        level + 1,
        partitionsByKey.get(key),
        'Filtrar partições',
        node =>
          pushTable(
            serverId,
            database,
            schema,
            node.name,
            schema ? openSizeFor(serverId, database, schema, node.name) : null,
            level + 1,
            node.kind,
          ),
      );
      return;
    }

    const colQuery = columnsByKey.get(key);
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

  const schemaActions = (serverId: number, database: string, schema: string) => [
    {
      label: 'Atualizar',
      icon: RefreshIcon,
      onSelect: () => void refreshSchema(serverId, database, schema),
    },
    {
      label: 'Nova aba',
      icon: Add01Icon,
      onSelect: () =>
        openQueryTab({ serverId, database, schema, title: schema }),
    },
  ];

  const openSizeFor = (serverId: number, database: string, schema: string, table: string) =>
    openRef?.serverId === serverId &&
    openRef.database === database &&
    openRef.schema === schema &&
    openRef.table === table
      ? (openSize.data ?? null)
      : null;

  /**
   * Database com catálogo: schemas em janela, relações sob demanda. Sem
   * schema (Mongo, SQLite), as relações vêm direto, também em janela.
   */
  const pushCatalogDatabase = (serverId: number, database: string, dbType: DatabaseType) => {
    const dbNodeId = encodeNodeId({ serverId, database });
    const key = `${serverId}|${database}`;
    const status = statusByKey.get(key);
    const children = dbChildrenByKey.get(key);

    if (status?.isError) {
      rows.push({
        variant: 'error',
        id: `${dbNodeId}|err`,
        level: 2,
        message: status.error.message,
        onRetry: () => void status.refetch(),
      });
      return;
    }

    // A sincronização falhou antes de qualquer schema chegar: sem lista,
    // o erro é o que há para mostrar
    const page = children?.data;
    const syncError = status?.data?.error;
    if (syncError && !page?.total && !status?.data?.stats.schemas) {
      rows.push({
        variant: 'error',
        id: `${dbNodeId}|sync-err`,
        level: 2,
        message: syncError,
        onRetry: () => void refreshDatabase(serverId, database),
      });
      return;
    }

    if (isFlatCatalog(dbType)) {
      const placeholder = dbType === 'mongodb' ? 'Filtrar coleções' : 'Filtrar tabelas';
      pushWindowed(dbNodeId, 2, children, placeholder, relation =>
        pushTable(
          serverId,
          database,
          null,
          relation.name,
          openSizeFor(serverId, database, FLAT_SCHEMA, relation.name),
          2,
          relation.kind,
        ),
      );
      return;
    }

    const shapes = shapesByKey.get(key);
    if (shapes?.isError) {
      rows.push({
        variant: 'error',
        id: `${dbNodeId}|shapes-err`,
        level: 2,
        message: shapes.error.message,
        onRetry: () => void shapes.refetch(),
      });
      return;
    }
    if (shapes && !shapes.data) return;
    if (shapes?.data?.length) {
      for (const group of shapes.data) pushShapeGroup(serverId, database, group);
      return;
    }

    pushWindowed(dbNodeId, 2, children, 'Filtrar schemas', node =>
      pushCatalogSchema(serverId, database, node, 2, true),
    );
  };

  /** Um grupo da árvore agrupada e, aberto, os schemas dele em janela */
  const pushShapeGroup = (serverId: number, database: string, group: ShapeGroup) => {
    const groupNodeId = encodeNodeId({ serverId, database, shape: group.key });
    const isGroupExpanded = expanded.has(groupNodeId);
    const schemas = shapeSchemasByKey.get(`${serverId}|${database}|${group.key}`);
    const { name, subLabel, badge } = shapeGroupRow(group);

    rows.push({
      variant: 'node',
      id: groupNodeId,
      props: {
        level: 2,
        kind: 'shape',
        name,
        subLabel,
        badge,
        hasChildren: group.schemas > 0,
        isExpanded: isGroupExpanded,
        isLoading: isGroupExpanded && (schemas?.isFetching ?? false),
        onClick: () => toggleNode(groupNodeId),
      },
    });

    if (!isGroupExpanded) return;
    // Dentro do grupo a diferença já está no próprio grupo: sem aviso por schema
    pushWindowed(groupNodeId, 3, schemas, 'Filtrar schemas', node =>
      pushCatalogSchema(serverId, database, node, 3, false),
    );
  };

  /** Um schema com catálogo e, aberto, as relações dele em janela */
  const pushCatalogSchema = (
    serverId: number,
    database: string,
    node: CatalogNode,
    level: number,
    showDrift: boolean,
  ) => {
    const schemaNodeId = encodeNodeId({ serverId, database, schema: node.name });
    const isSchemaExpanded = expanded.has(schemaNodeId);
    const relations = relationListByKey.get(`${serverId}|${database}|${node.name}`);
    // Com aviso de drift a contagem sai: a sidebar é estreita e o nome do
    // tenant não pode sumir
    const badge = showDrift ? driftBadge(node.drift) : undefined;

    rows.push({
      variant: 'node',
      id: schemaNodeId,
      props: {
        level,
        kind: 'schema',
        name: node.name,
        subLabel:
          badge || node.childCount == null ? undefined : formatCount(node.childCount),
        badge,
        // Ainda não carregado: pode ter filhos — abrir carrega na hora
        hasChildren: node.childCount == null || node.childCount > 0,
        isExpanded: isSchemaExpanded,
        isLoading: isSchemaExpanded && (relations?.isFetching ?? false),
        onClick: () => toggleNode(schemaNodeId),
        actions: schemaActions(serverId, database, node.name),
      },
    });

    if (!isSchemaExpanded) return;
    pushWindowed(schemaNodeId, level + 1, relations, 'Filtrar tabelas', relation =>
      pushTable(
        serverId,
        database,
        node.name,
        relation.name,
        openSizeFor(serverId, database, node.name, relation.name),
        level + 1,
        relation.kind,
      ),
    );
  };

  /** Copia o diagnóstico do catálogo (contagens e tempos, sem nomes) */
  const copyDiagnostics = async (
    serverId: number,
    database: string,
    dbType: DatabaseType,
  ) => {
    try {
      const data = await diagnostics.mutateAsync({ serverId, database });
      await writeText(formatCatalogDiagnostics(data, dbType));
      toast.success('Diagnóstico copiado.');
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error));
    }
  };

  /** Ações a mais de um database com catálogo */
  const catalogActions = (
    serverId: number,
    database: string,
    dbType: DatabaseType,
    status: CatalogStatus | undefined,
  ) => {
    const dbNodeId = encodeNodeId({ serverId, database });
    // Só vale agrupar quando há formato repetido (algum molde de tenant)
    const stats = status?.stats;
    const repeats =
      !!stats && stats.loaded + stats.stale > stats.shapes && stats.shapes > 0;
    const grouped = shapeGrouped.has(dbNodeId);
    return [
      ...(!isFlatCatalog(dbType) && (repeats || grouped)
        ? [
            {
              label: grouped ? 'Lista simples' : 'Agrupar por formato',
              icon: grouped ? ListViewIcon : LayoutGridIcon,
              onSelect: () => toggleShapeGrouping(dbNodeId),
            },
          ]
        : []),
      {
        label: 'Copiar diagnóstico',
        icon: Stethoscope02Icon,
        onSelect: () => void copyDiagnostics(serverId, database, dbType),
      },
    ];
  };

  for (const server of servers) {
    const serverNodeId = encodeNodeId({ serverId: server.id });
    const isExpanded = expanded.has(serverNodeId);
    const dbQuery = databasesByServer.get(server.id);
    const withCatalog = catalogServerIds.has(server.id);

    const databases = dbQuery?.data;

    rows.push({
      variant: 'node',
      id: serverNodeId,
      props: {
        level: 0,
        kind: 'server',
        name: server.name,
        // Quantos databases (o filtro aparece acima de 50)
        subLabel:
          isExpanded && databases && databases.length > FILTER_FROM && !filters.get(serverNodeId)
            ? formatCount(databases.length)
            : undefined,
        hasChildren: true,
        isExpanded,
        isLoading: isExpanded && (dbQuery?.isFetching ?? false),
        dbType: server.dbType,
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

    if (!databases) continue;

    // Um database por tenant: milhares de databases ganham filtro e janela
    const page = localPage(serverNodeId, databases, db => db.name);
    pushPage(serverNodeId, 1, page, false, 'Filtrar databases', db => {
      const dbNodeId = encodeNodeId({ serverId: server.id, database: db.name });
      const isDbExpanded = expanded.has(dbNodeId);
      const key = `${server.id}|${db.name}`;
      const structureQuery = structureByKey.get(key);
      const catalogStatus = statusByKey.get(key);
      const childList = dbChildrenByKey.get(key);
      const shapes = shapesByKey.get(key);
      // Agrupado não há lista: a contagem vem do estado do catálogo
      const childTotal =
        childList?.data?.total ??
        (shapes?.data?.length ? catalogStatus?.data?.stats.schemas : undefined);

      rows.push({
        variant: 'node',
        id: dbNodeId,
        props: {
          level: 1,
          kind: 'database',
          name: db.name,
          // Com catálogo: quantos schemas, ou relações quando não há schema
          // (o filtro aparece acima de 50)
          subLabel:
            withCatalog && isDbExpanded && childTotal && !filters.get(dbNodeId)
              ? formatCount(childTotal)
              : undefined,
          hasChildren: true,
          isExpanded: isDbExpanded,
          // Com catálogo, gira também durante a revalidação em segundo plano
          isLoading:
            isDbExpanded &&
            (withCatalog
              ? (catalogStatus?.data?.syncing ?? catalogStatus?.isFetching ?? false) ||
                (childList?.isFetching ?? false) ||
                (shapes?.isFetching ?? false)
              : (structureQuery?.isFetching ?? false)),
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
            ...(withCatalog
              ? catalogActions(
                  server.id,
                  db.name,
                  server.dbType,
                  catalogStatus?.data,
                )
              : []),
          ],
        },
      });

      if (!isDbExpanded) return;

      if (withCatalog) {
        pushCatalogDatabase(server.id, db.name, server.dbType);
        return;
      }

      if (structureQuery?.isError) {
        rows.push({
          variant: 'error',
          id: `${dbNodeId}|err`,
          level: 2,
          message: structureQuery.error.message,
          onRetry: () => void structureQuery.refetch(),
        });
        return;
      }

      const structure = structureQuery?.data;
      if (!structure) return;

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
              actions: schemaActions(server.id, db.name, schema.name),
            },
          });

          if (!isSchemaExpanded) continue;

          for (const table of schema.tables) {
            pushTable(
              server.id,
              db.name,
              schema.name,
              table.name,
              table.sizeBytes,
              3,
            );
          }
        }
      } else {
        for (const table of structure.schemas.flatMap(s => s.tables)) {
          pushTable(server.id, db.name, null, table.name, table.sizeBytes, 2);
        }
      }
    });
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
