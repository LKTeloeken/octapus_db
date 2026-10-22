import {
  Add01Icon,
  ColumnInsertIcon,
  DatabaseExportIcon,
  DatabaseImportIcon,
  Delete02Icon,
  SourceCodeIcon,
  Edit01Icon,
  FolderTransferIcon,
  GitCompareIcon,
  LayoutGridIcon,
  ListViewIcon,
  PencilEdit02Icon,
  RefreshIcon,
  Stethoscope02Icon,
  Table01Icon,
} from '@hugeicons/core-free-icons';
import { useQueries, type UseQueryResult } from '@tanstack/react-query';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { useMemo, useRef } from 'react';
import toast from 'react-hot-toast';
import { getCapabilities } from '@/api/connection';
import {
  listColumns,
  listDatabases,
  listSchemasWithTables,
} from '@/api/structure';
import type { BackupTarget } from '@/api/types/backup.types';
import type { AdapterCapabilities } from '@/api/types/capabilities.types';
import {
  FLAT_SCHEMA,
  type CatalogFolder,
  type CatalogNode,
  type CatalogNodeKind,
  type CatalogObject,
  type CatalogObjectKind,
  type CatalogObjectRef,
  type CatalogPage,
  type CatalogPath,
  type CatalogStatus,
  type ShapeGroup,
} from '@/api/types/catalog.types';
import type {
  DatabaseType,
  Server,
  ServerGroup,
} from '@/api/types/server.types';
import type {
  ColumnInfo,
  DatabaseInfo,
  DatabaseStructure,
} from '@/api/types/structure.types';
import { useServerGroups } from '@/queries/use-server-groups';
import { useServers } from '@/queries/use-servers';
import { useDbCapabilities } from '@/queries/use-capabilities';
import { useRefreshStructure } from '@/queries/use-refresh-structure';
import {
  CATALOG_PAGE_SIZE,
  catalogChildrenQuery,
  catalogFoldersQuery,
  catalogObjectsQuery,
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
import { useBackupDialogStore } from '@/stores/backup-dialog-store';
import { useServerGroupDialogStore } from '@/stores/server-group-dialog-store';
import { useFocusStore } from '@/stores/focus-store';
import {
  useStructureEditorStore,
  type StructureTarget,
} from '@/stores/structure-editor-store';
import { useTabsStore } from '@/stores/tabs-store';
import { useTreeStore } from '@/stores/tree-store';
import { formatCatalogDiagnostics } from '@/lib/catalog-diagnostics';
import { DEFAULT_DATABASES } from '@/lib/db-defaults';
import { formatCount } from '@/lib/format-count';
import {
  decodeNodeId,
  encodeNodeId,
  groupNodeId,
  nodeKind,
  type NodeRef,
} from '@/lib/node-ref';
import type { ConnectionTreeProps, FlatRow } from './connection-tree.types';
import type { NodeRowAction, TreeFolderKind } from './node-row/node-row.types';
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

/** Relações que têm colunas próprias para editar (views e externas não) */
const hasEditableColumns = (kind: CatalogNodeKind | undefined) =>
  kind === undefined || kind === 'table' || kind === 'partitioned';

/** Há formato repetido entre os schemas (algum molde de tenant): só aí vale agrupar */
const hasRepeatedShapes = (status: CatalogStatus | undefined) => {
  const stats = status?.stats;
  return (
    !!stats && stats.loaded + stats.stale > stats.shapes && stats.shapes > 0
  );
};
const schemaKey = (ref: NodeRef) =>
  `${ref.serverId}|${ref.database}|${ref.schema}`;

/** Nome de cada pasta de objetos e o texto do filtro dela */
const FOLDERS: Record<CatalogObjectKind, { label: string; filter: string }> = {
  view: { label: 'Views', filter: 'Filtrar views' },
  materializedView: {
    label: 'Views materializadas',
    filter: 'Filtrar views materializadas',
  },
  index: { label: 'Índices', filter: 'Filtrar índices' },
  function: { label: 'Funções', filter: 'Filtrar funções' },
  sequence: { label: 'Sequences', filter: 'Filtrar sequences' },
  trigger: { label: 'Triggers', filter: 'Filtrar triggers' },
  foreignKey: {
    label: 'Chaves estrangeiras',
    filter: 'Filtrar chaves estrangeiras',
  },
};

/** Pastas cuja contagem o catálogo já sabe: vazias, nem aparecem */
const COUNTED_FOLDERS: ReadonlySet<CatalogObjectKind> =
  new Set<CatalogObjectKind>(['view', 'materializedView']);

/** O que a pasta aberta e vazia diz */
const EMPTY_FOLDER: Record<CatalogObjectKind, string> = {
  view: 'Nenhuma view',
  materializedView: 'Nenhuma view materializada',
  index: 'Nenhum índice',
  function: 'Nenhuma função',
  sequence: 'Nenhuma sequence',
  trigger: 'Nenhum trigger',
  foreignKey: 'Nenhuma chave estrangeira',
};

/** Relações com índices próprios (pasta Índices dentro delas) */
const hasIndexes = (kind: CatalogNodeKind | undefined) =>
  kind === undefined ||
  kind === 'table' ||
  kind === 'partitioned' ||
  kind === 'materializedView';

/**
 * A pasta de objetos vale para este tipo de relação? Índices também na view
 * materializada; triggers e chaves estrangeiras só em tabela (comum ou
 * particionada).
 */
const relationFolderApplies = (
  folder: CatalogObjectKind,
  relation: CatalogNodeKind | undefined,
) =>
  folder === 'index'
    ? hasIndexes(relation)
    : relation === undefined ||
      relation === 'table' ||
      relation === 'partitioned';

/** A pasta das tabelas de um schema (`schema` null = o schema sem nome) */
const tablesFolderId = (
  serverId: number,
  database: string,
  schema: string | null,
) =>
  encodeNodeId({
    serverId,
    database,
    schema: schema ?? undefined,
    folder: 'tables',
  });

const FUNCTION_KINDS = {
  function: 'função',
  procedure: 'procedure',
  aggregate: 'agregada',
  window: 'janela',
} as const;

const isViewKind = (kind: CatalogNodeKind | undefined) =>
  kind === 'view' || kind === 'materializedView';

/** Pastas do schema (`structureFolders`): vazio enquanto as capabilities carregam */
const structureFoldersOf = (caps: AdapterCapabilities | undefined) =>
  caps?.structureFolders ?? [];

/**
 * As relações soltas de um schema: com pasta de views, elas saem da lista
 * (`tables`); sem, a lista é a de sempre.
 */
const loosePath = (
  caps: AdapterCapabilities | undefined,
  schema: string,
): CatalogPath =>
  structureFoldersOf(caps).includes('view')
    ? { kind: 'tables', schema }
    : { kind: 'schema', schema };

const folderKey = (ref: NodeRef) =>
  `${ref.serverId}|${ref.database}|${ref.schema ?? FLAT_SCHEMA}|${ref.table ?? ''}|${ref.folder}`;

/**
 * O nome e o detalhe de um item de pasta. `inRelation`: a pasta é a de uma
 * tabela (o nome dela não se repete no detalhe).
 */
function objectLabels(
  object: Exclude<CatalogObject, { kind: 'view' | 'materializedView' }>,
  inRelation: boolean,
): { name: string; subLabel: string } {
  const join = (parts: (string | null | false)[]) =>
    parts.filter(Boolean).join(' · ');
  switch (object.kind) {
    case 'index':
      return { name: object.name, subLabel: indexLabel(object, inRelation) };
    case 'function':
      return {
        name: `${object.name}(${object.signature})`,
        subLabel: join([
          object.returns ? `→ ${object.returns}` : null,
          FUNCTION_KINDS[object.functionKind],
          object.language,
        ]),
      };
    case 'sequence':
      return {
        name: object.name,
        subLabel: join([object.dataType, object.ownedBy]),
      };
    case 'trigger':
      return {
        name: object.name,
        subLabel: join([
          !inRelation && object.table,
          `${object.timing} ${object.events.join(' OR ')}`,
          object.function,
          !object.enabled && 'desligado',
        ]),
      };
    case 'foreignKey': {
      const to = object.refColumns.length
        ? `${object.refTable}(${object.refColumns.join(', ')})`
        : object.refTable;
      return {
        name: object.name,
        subLabel: join([
          `${inRelation ? '' : object.table}(${object.columns.join(', ')}) → ${to}`,
          object.onDelete !== 'NO ACTION' && `ON DELETE ${object.onDelete}`,
        ]),
      };
    }
  }
}

/** O detalhe de um índice: tabela (fora da pasta da coleção), colunas, método */
function indexLabel(
  index: Extract<CatalogObject, { kind: 'index' }>,
  inRelation: boolean,
) {
  const columns = `(${index.columns.join(', ')})`;
  const role = index.primary ? ' · primário' : index.unique ? ' · único' : '';
  const partial = index.predicate ? ' · parcial' : '';
  return `${inRelation ? columns : `${index.table} ${columns}`} · ${index.method}${role}${partial}`;
}

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
export const useConnectionTree = ({
  onEditServer,
  onCreateServer,
}: ConnectionTreeProps) => {
  useCatalogEvents();

  const serversQuery = useServers();
  // O que cada banco suporta (catálogo, edição de estrutura, backup…)
  const capsByType = useDbCapabilities();
  const groupsQuery = useServerGroups();
  const collapsedGroups = useTreeStore(state => state.collapsedGroups);
  const openGroupDialog = useServerGroupDialogStore(state => state.open);
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
  const openDefinitionTab = useTabsStore(state => state.openDefinitionTab);
  const openBrowseTab = useTabsStore(state => state.openBrowseTab);
  // A aba de tabela tem o mesmo id do nó da árvore: basta comparar.
  const activeTabId = useTabsStore(state => state.activeTabId);
  const requestFocus = useFocusStore(state => state.requestFocus);
  const openStructureDialog = useStructureEditorStore(state => state.open);
  const openBackupDialog = useBackupDialogStore(state => state.open);
  const { refreshServer, refreshDatabase, refreshSchema, refreshTable } =
    useRefreshStructure();
  // Última janela de cada nó. O `keepPreviousData` não vale no `useQueries`:
  // trocar o filtro troca a key, o observer é outro e a página some até a nova
  // chegar — junto com a linha do filtro, que sai do DOM e leva o foco do
  // campo a cada letra digitada.
  const lastPages = useRef(new Map<string, CatalogPage<unknown>>());

  const servers = useMemo(() => serversQuery.data ?? [], [serversQuery.data]);
  /** O "Editar servidor" dos detalhes de um erro de conexão */
  const editServer = (serverId: number) => {
    const server = servers.find(item => item.id === serverId);
    return server ? () => onEditServer(server) : undefined;
  };
  const serverIds = useMemo(
    () => new Set(servers.map(server => server.id)),
    [servers],
  );
  const catalogServerIds = useMemo(
    () =>
      new Set(
        servers
          .filter(server => hasCatalog(capsByType[server.dbType]))
          .map(server => server.id),
      ),
    [servers, capsByType],
  );
  const dbTypes = useMemo(
    () => new Map(servers.map(server => [server.id, server.dbType])),
    [servers],
  );
  const flatServerIds = useMemo(
    () =>
      new Set(
        servers
          .filter(server => isFlatCatalog(capsByType[server.dbType]))
          .map(server => server.id),
      ),
    [servers, capsByType],
  );

  // Decode the expansion set into the entities whose children must be fetched.
  const expandedRefs = useMemo(
    () => Array.from(expanded).map(decodeNodeId),
    [expanded],
  );
  const expandedServerIds = useMemo(
    () =>
      expandedRefs
        .filter(
          ref => nodeKind(ref) === 'server' && serverIds.has(ref.serverId),
        )
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
  // Estrutura inteira (Redis) só com as capabilities já em mãos: enquanto
  // carregam, um Postgres pareceria "sem catálogo" e pediria a estrutura
  // inteira — o que derrubava servidores com milhares de schemas
  const legacyServerIds = useMemo(
    () =>
      new Set(
        servers
          .filter(server => {
            const caps = capsByType[server.dbType];
            return caps !== undefined && !caps.hasCatalog;
          })
          .map(server => server.id),
      ),
    [servers, capsByType],
  );
  const legacyDbRefs = useMemo(
    () => expandedDbRefs.filter(ref => legacyServerIds.has(ref.serverId)),
    [expandedDbRefs, legacyServerIds],
  );
  const catalogDbRefs = useMemo(
    () =>
      expandedDbRefs.filter(
        ref =>
          catalogServerIds.has(ref.serverId) &&
          !flatServerIds.has(ref.serverId),
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
          expanded.has(
            encodeNodeId({ serverId: ref.serverId, database: ref.database }),
          ),
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
    queries: catalogStatusRefs.map(ref =>
      catalogStatusQuery(ref.serverId, ref.database!),
    ),
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
  groupedDbRefs.forEach((ref, i) =>
    shapesByKey.set(dbKey(ref), shapeQueries[i]),
  );
  /** Sem formato repetido não há grupos: a árvore fica na lista simples */
  const listDbRefs = catalogDbRefs.filter(ref => {
    const shapes = shapesByKey.get(dbKey(ref));
    return !shapes || shapes.data?.length === 0;
  });

  const expandedShapeRefs = expandedRefs.filter(ref => {
    if (nodeKind(ref) !== 'shape') return false;
    const groups = shapesByKey.get(dbKey(ref))?.data;
    return (
      expanded.has(
        encodeNodeId({ serverId: ref.serverId, database: ref.database }),
      ) && !!groups?.some(group => group.key === ref.shape)
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
      const { filter, limit } = windowOf(
        tablesFolderId(ref.serverId, ref.database!, null),
      );
      return catalogChildrenQuery(
        ref.serverId,
        ref.database!,
        loosePath(capsByType[dbTypes.get(ref.serverId)!], FLAT_SCHEMA),
        filter,
        limit,
      );
    }),
  });

  const relationListQueries = useQueries({
    queries: catalogSchemaRefs.map(ref => {
      const { filter, limit } = windowOf(
        tablesFolderId(ref.serverId, ref.database!, ref.schema!),
      );
      return catalogChildrenQuery(
        ref.serverId,
        ref.database!,
        loosePath(capsByType[dbTypes.get(ref.serverId)!], ref.schema!),
        filter,
        limit,
      );
    }),
  });

  // Pastas (views, índices, funções) dos schemas abertos — no Mongo e no
  // SQLite, do schema sem nome direto sob o database
  const folderSchemaRefs = [
    ...catalogSchemaRefs,
    ...flatDbRefs.map(ref => ({ ...ref, schema: FLAT_SCHEMA })),
  ].filter(
    ref =>
      structureFoldersOf(capsByType[dbTypes.get(ref.serverId)!]).length > 0,
  );
  const folderListQueries = useQueries({
    queries: folderSchemaRefs.map(ref =>
      catalogFoldersQuery(ref.serverId, ref.database!, ref.schema!),
    ),
  });
  const foldersByKey = new Map<string, UseQueryResult<CatalogFolder[]>>();
  folderSchemaRefs.forEach((ref, i) =>
    foldersByKey.set(schemaKey(ref), folderListQueries[i]),
  );

  // Pasta aberta e visível: a do schema (ou do database, sem schema) ou a da
  // tabela aberta (índices). Só ela pede os objetos ao backend
  const expandedFolderRefs = expandedRefs.filter(ref => {
    if (nodeKind(ref) !== 'folder' || !catalogServerIds.has(ref.serverId)) {
      return false;
    }
    const caps = capsByType[dbTypes.get(ref.serverId)!];
    const dbOpen = expanded.has(
      encodeNodeId({ serverId: ref.serverId, database: ref.database }),
    );
    const kind = ref.folder as CatalogObjectKind;
    if (ref.table != null) {
      return (
        dbOpen &&
        !!caps?.relationFolders.includes(kind) &&
        expanded.has(
          encodeNodeId({
            serverId: ref.serverId,
            database: ref.database,
            schema: ref.schema,
            table: ref.table,
          }),
        )
      );
    }
    const parentOpen =
      ref.schema == null
        ? flatServerIds.has(ref.serverId)
        : expanded.has(
            encodeNodeId({
              serverId: ref.serverId,
              database: ref.database,
              schema: ref.schema,
            }),
          );
    return dbOpen && parentOpen && structureFoldersOf(caps).includes(kind);
  });
  const objectQueries = useQueries({
    queries: expandedFolderRefs.map(ref => {
      const { filter, limit } = windowOf(encodeNodeId(ref));
      return catalogObjectsQuery(
        ref.serverId,
        ref.database!,
        ref.schema ?? FLAT_SCHEMA,
        ref.folder as CatalogObjectKind,
        ref.table ?? null,
        filter,
        limit,
      );
    }),
  });
  const objectsByKey = new Map<
    string,
    UseQueryResult<CatalogPage<CatalogObject>>
  >();
  expandedFolderRefs.forEach((ref, i) =>
    objectsByKey.set(folderKey(ref), objectQueries[i]),
  );

  // Tipo de cada relação já listada: uma particionada também tem a pasta das
  // partições
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
  // Colunas de toda tabela aberta: a pasta Colunas já mostra quantas são
  const columnRefs = expandedTableRefs;

  const partitionQueries = useQueries({
    queries: partitionedRefs.map(ref => {
      const { filter, limit } = windowOf(
        encodeNodeId({ ...ref, folder: 'partitions' }),
      );
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
  expandedServerIds.forEach((id, i) =>
    databasesByServer.set(id, databaseQueries[i]),
  );

  const capabilitiesByServer = new Map<
    number,
    UseQueryResult<AdapterCapabilities>
  >();
  expandedServerIds.forEach((id, i) =>
    capabilitiesByServer.set(id, capabilityQueries[i]),
  );

  const structureByKey = new Map<string, UseQueryResult<DatabaseStructure>>();
  legacyDbRefs.forEach((ref, i) =>
    structureByKey.set(dbKey(ref), structureQueries[i]),
  );

  const statusByKey = new Map<string, UseQueryResult<CatalogStatus>>();
  catalogStatusRefs.forEach((ref, i) =>
    statusByKey.set(dbKey(ref), catalogStatusQueries[i]),
  );

  // Filhos do database com catálogo: schemas ou, sem schema, as relações
  const dbChildrenByKey = new Map<
    string,
    UseQueryResult<CatalogPage<CatalogNode>>
  >();
  listDbRefs.forEach((ref, i) =>
    dbChildrenByKey.set(dbKey(ref), schemaListQueries[i]),
  );
  flatDbRefs.forEach((ref, i) =>
    dbChildrenByKey.set(dbKey(ref), flatListQueries[i]),
  );

  const relationListByKey = new Map<
    string,
    UseQueryResult<CatalogPage<CatalogNode>>
  >();
  catalogSchemaRefs.forEach((ref, i) =>
    relationListByKey.set(schemaKey(ref), relationListQueries[i]),
  );

  const partitionsByKey = new Map<
    string,
    UseQueryResult<CatalogPage<CatalogNode>>
  >();
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
  const pushPage = <T>(
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
  const pushWindowed = <T = CatalogNode>(
    nodeId: string,
    level: number,
    query: UseQueryResult<CatalogPage<T>> | undefined,
    placeholder: string,
    pushChild: (node: T) => void,
  ) => {
    if (query?.isError) {
      rows.push({
        variant: 'error',
        id: `${nodeId}|err`,
        level,
        message: query.error.message,
        error: query.error,
        onRetry: () => void query.refetch(),
      });
      return;
    }

    if (query?.data) lastPages.current.set(nodeId, query.data);
    const page =
      query?.data ??
      (query?.isPending
        ? (lastPages.current.get(nodeId) as CatalogPage<T> | undefined)
        : undefined);
    if (!query || !page) return;
    pushPage(nodeId, level, page, query.isFetching, placeholder, pushChild);
  };

  /** A mesma janela sobre uma lista que já está inteira no front */
  const localPage = <T>(
    nodeId: string,
    items: T[],
    nameOf: (item: T) => string,
  ) => {
    const needle = (filters.get(nodeId) ?? '').trim().toLowerCase();
    const matching = needle
      ? items.filter(item => nameOf(item).toLowerCase().includes(needle))
      : items;
    const limit = limits.get(nodeId) ?? CATALOG_PAGE_SIZE;
    return {
      total: matching.length,
      offset: 0,
      items: matching.slice(0, limit),
    };
  };

  /** Onde a árvore edita a estrutura; null nos bancos sem edição */
  const structureTarget = (
    serverId: number,
    database: string,
    schema: string | null,
  ): StructureTarget | null => {
    const dbType = dbTypes.get(serverId);
    return dbType && capsByType[dbType]?.supportsStructureChanges
      ? { serverId, database, dbType, schema }
      : null;
  };

  /** Nova coluna, renomear e excluir, no menu de uma tabela */
  const tableStructureActions = (
    target: StructureTarget | null,
    table: string,
    relationKind: CatalogNodeKind | undefined,
  ): NodeRowAction[] => {
    if (!target) return [];
    const isTable = hasEditableColumns(relationKind);
    const isView =
      relationKind === 'view' || relationKind === 'materializedView';
    return [
      ...(isTable
        ? [
            {
              label: 'Nova coluna',
              icon: ColumnInsertIcon,
              separated: true,
              onSelect: () =>
                openStructureDialog({ kind: 'addColumn', target, table }),
            },
            {
              label: 'Renomear',
              icon: PencilEdit02Icon,
              onSelect: () =>
                openStructureDialog({ kind: 'renameTable', target, table }),
            },
          ]
        : []),
      {
        label: isView ? 'Excluir view' : 'Excluir tabela',
        icon: Delete02Icon,
        danger: true,
        separated: true,
        onSelect: () =>
          openStructureDialog({
            kind: 'dropTable',
            target,
            table,
            relationKind,
          }),
      },
    ];
  };

  /** O que o banco do servidor suporta; undefined enquanto carrega */
  const capsOf = (serverId: number) => {
    const dbType = dbTypes.get(serverId);
    return dbType ? capsByType[dbType] : undefined;
  };

  /** Backup pelo pg_dump do nó (database, schema ou tabela) */
  const backupAction = (
    serverId: number,
    database: string,
    target: BackupTarget,
  ): NodeRowAction[] =>
    capsOf(serverId)?.supportsBackup
      ? [
          {
            label: 'Fazer backup…',
            icon: DatabaseExportIcon,
            separated: true,
            onSelect: () =>
              openBackupDialog({ kind: 'backup', serverId, database, target }),
          },
        ]
      : [];

  /** Comparar dois schemas (aberto num schema, ele já é a base) */
  const compareAction = (
    serverId: number,
    database: string,
    base: string | null,
  ): NodeRowAction => ({
    label: 'Comparar schemas',
    icon: GitCompareIcon,
    onSelect: () =>
      openStructureDialog({ kind: 'compareSchemas', serverId, database, base }),
  });

  /**
   * "Ver definição": abre (ou ativa) a aba de definição do objeto — só o
   * texto, com destaque; de lá, "Abrir em aba de query" para editar e executar.
   */
  const openDefinition = (
    serverId: number,
    database: string,
    schema: string | null,
    target: CatalogObjectRef,
    title: string,
  ) => {
    openDefinitionTab({ serverId, database, schema, target, title });
  };

  const definitionAction = (onSelect: () => void): NodeRowAction => ({
    label: 'Ver definição',
    icon: SourceCodeIcon,
    onSelect,
  });

  /**
   * Um item de pasta que não é relação (índice, função, sequence, trigger,
   * chave estrangeira): o clique abre a definição numa aba de definição.
   * `table` = a pasta é a de uma tabela (índices, triggers, chaves; no Mongo,
   * a coleção).
   */
  const pushObject = (
    serverId: number,
    database: string,
    schema: string | null,
    folderNodeId: string,
    object: Exclude<CatalogObject, { kind: 'view' | 'materializedView' }>,
    level: number,
    table: string | null,
  ) => {
    const { name, subLabel } = objectLabels(object, table !== null);
    const target: CatalogObjectRef = {
      kind: object.kind,
      schema: schema ?? FLAT_SCHEMA,
      name: object.name,
      // O índice do Mongo é por coleção; trigger e chave, sempre da tabela
      table:
        object.kind === 'trigger' || object.kind === 'foreignKey'
          ? object.table
          : object.kind === 'index' && table !== null
            ? table
            : null,
      signature: object.kind === 'function' ? object.signature : null,
    };
    const open = () =>
      openDefinition(serverId, database, schema, target, object.name);
    const actions: NodeRowAction[] = [definitionAction(open)];
    if (object.kind === 'foreignKey') {
      const refSchema =
        object.refSchema === FLAT_SCHEMA ? null : object.refSchema;
      actions.unshift({
        label: 'Abrir tabela referenciada',
        icon: Table01Icon,
        onSelect: () =>
          openTable(serverId, database, refSchema, object.refTable),
      });
    }

    rows.push({
      variant: 'node',
      // Sobrecargas têm o mesmo nome: a assinatura entra no id
      id: `${folderNodeId}|ob=${encodeURIComponent(name)}`,
      props: {
        level,
        kind: object.kind,
        name,
        subLabel,
        // O comentário da função (`COMMENT ON FUNCTION`) só no passar do mouse
        title:
          object.kind === 'function' && object.comment
            ? `${name} ${subLabel}\n${object.comment}`
            : `${name} ${subLabel}`,
        hasChildren: false,
        onClick: open,
        actions,
      },
    });
  };

  /**
   * A linha de uma pasta (`count` null = ainda não se sabe quantos são).
   * Devolve se ela está aberta.
   */
  const pushFolderRow = (
    id: string,
    level: number,
    folderKind: TreeFolderKind,
    name: string,
    count: number | null,
    isLoading: boolean,
    actions?: NodeRowAction[],
  ) => {
    const isExpanded = expanded.has(id);
    rows.push({
      variant: 'node',
      id,
      props: {
        level,
        kind: 'folder',
        folderKind,
        name,
        subLabel: count == null ? undefined : formatCount(count),
        hasChildren: true,
        isExpanded,
        isLoading: isExpanded && isLoading,
        onClick: () => toggleNode(id),
        actions,
      },
    });
    return isExpanded;
  };

  /**
   * Uma pasta de objetos (views, índices, funções) e, aberta, a janela dos
   * itens dela. Sem `table`, a do schema (`schema` null = o schema sem nome do
   * Mongo e do SQLite); com `table`, a da tabela (índices). Views vazias nem
   * aparecem (o catálogo sabe quantas são); índices e funções só se sabem ao
   * abrir, e a pasta aberta e vazia diz isso em vez de sumir.
   */
  const pushFolder = (
    serverId: number,
    database: string,
    schema: string | null,
    kind: CatalogObjectKind,
    count: number | null,
    level: number,
    table: string | null = null,
  ) => {
    const ref: NodeRef = {
      serverId,
      database,
      schema: schema ?? undefined,
      table: table ?? undefined,
      folder: kind,
    };
    const folderNodeId = encodeNodeId(ref);
    const objects = objectsByKey.get(folderKey(ref));
    const filter = filters.get(folderNodeId) ?? '';
    // Aberta e lida sem filtro, a própria janela diz quantos são
    const known =
      !filter && objects?.data ? objects.data.total : (count ?? null);
    if (known === 0 && COUNTED_FOLDERS.has(kind)) return;

    const isExpanded = pushFolderRow(
      folderNodeId,
      level,
      kind,
      FOLDERS[kind].label,
      known,
      objects?.isFetching ?? false,
      table === null && schema !== null
        ? [
            {
              label: 'Atualizar',
              icon: RefreshIcon,
              onSelect: () => void refreshSchema(serverId, database, schema),
            },
          ]
        : undefined,
    );

    if (!isExpanded) return;
    if (known === 0 && !filter) {
      rows.push({
        variant: 'empty',
        id: `${folderNodeId}|empty`,
        level: level + 1,
        message: EMPTY_FOLDER[kind],
      });
      return;
    }
    pushWindowed(
      folderNodeId,
      level + 1,
      objects,
      FOLDERS[kind].filter,
      item => {
        if (item.kind === 'view' || item.kind === 'materializedView') {
          pushTable(
            serverId,
            database,
            schema,
            item.name,
            openSizeFor(serverId, database, schema ?? FLAT_SCHEMA, item.name),
            level + 1,
            item.kind,
          );
        } else {
          pushObject(
            serverId,
            database,
            schema,
            folderNodeId,
            item,
            level + 1,
            table,
          );
        }
      },
    );
  };

  /**
   * A pasta das tabelas (coleções, no Mongo) de um schema aberto e, aberta, a
   * janela delas. Sem tabela nenhuma, a pasta não aparece.
   */
  const pushTablesFolder = (
    serverId: number,
    database: string,
    schema: string | null,
    relations: UseQueryResult<CatalogPage<CatalogNode>> | undefined,
    level: number,
  ) => {
    const folderNodeId = tablesFolderId(serverId, database, schema);
    const filter = filters.get(folderNodeId) ?? '';
    const count = !filter && relations?.data ? relations.data.total : null;
    if (count === 0) return;
    const isMongo = dbTypes.get(serverId) === 'mongodb';

    const isExpanded = pushFolderRow(
      folderNodeId,
      level,
      'tables',
      isMongo ? 'Coleções' : 'Tabelas',
      count,
      relations?.isFetching ?? false,
    );
    if (!isExpanded) return;
    pushWindowed(
      folderNodeId,
      level + 1,
      relations,
      isMongo ? 'Filtrar coleções' : 'Filtrar tabelas',
      relation =>
        pushTable(
          serverId,
          database,
          schema,
          relation.name,
          openSizeFor(serverId, database, schema ?? FLAT_SCHEMA, relation.name),
          level + 1,
          relation.kind,
        ),
    );
  };

  /** As pastas de objetos de um schema aberto, depois da pasta das tabelas */
  const pushFolders = (
    serverId: number,
    database: string,
    schema: string | null,
    level: number,
  ) => {
    const key = `${serverId}|${database}|${schema ?? FLAT_SCHEMA}`;
    const query = foldersByKey.get(key);
    if (query?.isError) {
      rows.push({
        variant: 'error',
        id: `${key}|folders-err`,
        level,
        message: query.error.message,
        error: query.error,
        onRetry: () => void query.refetch(),
      });
      return;
    }
    for (const folder of query?.data ?? []) {
      pushFolder(serverId, database, schema, folder.kind, folder.count, level);
    }
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
    const childQuery = columnsByKey.get(key);
    const target = structureTarget(serverId, database, schema);
    const columnsEditable = !!target && hasEditableColumns(relationKind);

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
          ...(isViewKind(relationKind)
            ? [
                definitionAction(() =>
                  openDefinition(
                    serverId,
                    database,
                    schema,
                    {
                      kind: relationKind as CatalogObjectKind,
                      schema: schema ?? FLAT_SCHEMA,
                      name: table,
                      table: null,
                      signature: null,
                    },
                    table,
                  ),
                ),
              ]
            : []),
          ...(schema
            ? backupAction(serverId, database, { kind: 'table', schema, table })
            : []),
          ...tableStructureActions(target, table, relationKind),
        ],
      },
      // Pelo teclado o Enter só abre a tabela: expandir/colapsar as colunas já
      // é papel das setas ←/→.
      onEnter: () => openTable(serverId, database, schema, table),
    });

    if (!isExpanded) return;

    const caps = capsOf(serverId);
    const colQuery = columnsByKey.get(key);
    const pushColumnsUnder = (parentId: string, columnLevel: number) =>
      pushColumns(
        parentId,
        columnLevel,
        colQuery,
        table,
        columnsEditable ? target : null,
      );

    // Sem catálogo (Redis) e na view comum, só há colunas: vêm direto
    if (!caps?.hasCatalog || relationKind === 'view') {
      pushColumnsUnder(tableNodeId, level + 1);
      return;
    }

    // Colunas, índices e, na particionada, as partições: cada um na sua pasta
    const tableRef = {
      serverId,
      database,
      schema: schema ?? undefined,
      table,
    };
    const columnsFolderId = encodeNodeId({ ...tableRef, folder: 'columns' });
    const columnsOpen = pushFolderRow(
      columnsFolderId,
      level + 1,
      'columns',
      dbTypes.get(serverId) === 'mongodb' ? 'Campos' : 'Colunas',
      colQuery?.data?.length ?? null,
      colQuery?.isFetching ?? false,
    );
    if (columnsOpen) pushColumnsUnder(columnsFolderId, level + 2);

    for (const kind of caps.relationFolders) {
      if (relationFolderApplies(kind, relationKind)) {
        pushFolder(serverId, database, schema, kind, null, level + 1, table);
      }
    }

    if (!partitioned) return;
    const partitionsFolderId = encodeNodeId({
      ...tableRef,
      folder: 'partitions',
    });
    const partitions = partitionsByKey.get(key);
    const partitionsFilter = filters.get(partitionsFolderId) ?? '';
    const partitionsOpen = pushFolderRow(
      partitionsFolderId,
      level + 1,
      'partitions',
      'Partições',
      !partitionsFilter && partitions?.data ? partitions.data.total : null,
      partitions?.isFetching ?? false,
    );
    if (!partitionsOpen) return;
    pushWindowed(
      partitionsFolderId,
      level + 2,
      partitions,
      'Filtrar partições',
      node =>
        pushTable(
          serverId,
          database,
          schema,
          node.name,
          schema ? openSizeFor(serverId, database, schema, node.name) : null,
          level + 2,
          node.kind,
        ),
    );
  };

  /** As colunas de uma tabela sob `parentId` (a pasta Colunas ou a tabela) */
  const pushColumns = (
    parentId: string,
    level: number,
    colQuery: UseQueryResult<ColumnInfo[]> | undefined,
    table: string,
    target: StructureTarget | null,
  ) => {
    if (colQuery?.isError) {
      rows.push({
        variant: 'error',
        id: `${parentId}|err`,
        level,
        message: colQuery.error.message,
        error: colQuery.error,
        onRetry: () => void colQuery.refetch(),
      });
      return;
    }
    for (const column of colQuery?.data ?? []) {
      rows.push({
        variant: 'node',
        id: `${parentId}|c=${column.name}`,
        props: {
          level,
          kind: 'column',
          name: column.name,
          subLabel: `${column.dataType}${column.isPrimaryKey ? ' (PK)' : ''}`,
          hasChildren: false,
          actions: target
            ? [
                {
                  label: 'Editar coluna',
                  icon: PencilEdit02Icon,
                  onSelect: () =>
                    openStructureDialog({
                      kind: 'editColumn',
                      target,
                      table,
                      column,
                    }),
                },
                {
                  label: 'Excluir coluna',
                  icon: Delete02Icon,
                  danger: true,
                  separated: true,
                  onSelect: () =>
                    openStructureDialog({
                      kind: 'dropColumn',
                      target,
                      table,
                      column: column.name,
                    }),
                },
              ]
            : undefined,
        },
      });
    }
  };

  const schemaActions = (
    serverId: number,
    database: string,
    schema: string,
  ): NodeRowAction[] => {
    const target = structureTarget(serverId, database, schema);
    return [
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
      ...(target
        ? [
            {
              label: 'Nova tabela',
              icon: Table01Icon,
              separated: true,
              onSelect: () =>
                openStructureDialog({ kind: 'createTable', target }),
            },
          ]
        : []),
      ...(capsOf(serverId)?.supportsSchemaCompare
        ? [compareAction(serverId, database, schema)]
        : []),
      ...backupAction(serverId, database, { kind: 'schema', schema }),
    ];
  };

  const openSizeFor = (
    serverId: number,
    database: string,
    schema: string,
    table: string,
  ) =>
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
  const pushCatalogDatabase = (
    serverId: number,
    database: string,
    dbType: DatabaseType,
  ) => {
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
        error: status.error,
        onRetry: () => void status.refetch(),
        onEdit: editServer(serverId),
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
        onEdit: editServer(serverId),
      });
      return;
    }

    if (isFlatCatalog(capsByType[dbType])) {
      pushTablesFolder(serverId, database, null, children, 2);
      pushFolders(serverId, database, null, 2);
      return;
    }

    const shapes = shapesByKey.get(key);
    if (shapes?.isError) {
      rows.push({
        variant: 'error',
        id: `${dbNodeId}|shapes-err`,
        level: 2,
        message: shapes.error.message,
        error: shapes.error,
        onRetry: () => void shapes.refetch(),
      });
      return;
    }
    if (shapes && !shapes.data) return;
    if (shapes?.data?.length) {
      for (const group of shapes.data)
        pushShapeGroup(serverId, database, group);
      return;
    }

    pushWindowed(dbNodeId, 2, children, 'Filtrar schemas', node =>
      pushCatalogSchema(serverId, database, node, 2, true),
    );
  };

  /** Um grupo da árvore agrupada e, aberto, os schemas dele em janela */
  const pushShapeGroup = (
    serverId: number,
    database: string,
    group: ShapeGroup,
  ) => {
    const groupNodeId = encodeNodeId({ serverId, database, shape: group.key });
    const isGroupExpanded = expanded.has(groupNodeId);
    const schemas = shapeSchemasByKey.get(
      `${serverId}|${database}|${group.key}`,
    );
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

  /** Um schema com catálogo e, aberto, a pasta das tabelas e as de objetos */
  const pushCatalogSchema = (
    serverId: number,
    database: string,
    node: CatalogNode,
    level: number,
    showDrift: boolean,
  ) => {
    const schemaNodeId = encodeNodeId({
      serverId,
      database,
      schema: node.name,
    });
    const isSchemaExpanded = expanded.has(schemaNodeId);
    const relations = relationListByKey.get(
      `${serverId}|${database}|${node.name}`,
    );
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
          badge || node.childCount == null
            ? undefined
            : formatCount(node.childCount),
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
    pushTablesFolder(serverId, database, node.name, relations, level + 1);
    pushFolders(serverId, database, node.name, level + 1);
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
    const repeats = hasRepeatedShapes(status);
    const grouped = shapeGrouped.has(dbNodeId);
    // Sem schema (SQLite), a tabela nova nasce direto no database
    const caps = capsByType[dbType];
    const flatTarget = isFlatCatalog(caps)
      ? structureTarget(serverId, database, null)
      : null;
    return [
      ...(!isFlatCatalog(caps) && (repeats || grouped)
        ? [
            {
              label: grouped ? 'Lista simples' : 'Agrupar por formato',
              icon: grouped ? ListViewIcon : LayoutGridIcon,
              onSelect: () => toggleShapeGrouping(dbNodeId),
            },
          ]
        : []),
      ...(caps?.supportsSchemaCompare
        ? [compareAction(serverId, database, null)]
        : []),
      ...backupAction(serverId, database, { kind: 'database' }),
      ...(caps?.supportsBackup
        ? [
            {
              label: 'Restaurar backup…',
              icon: DatabaseImportIcon,
              onSelect: () =>
                openBackupDialog({ kind: 'restore', serverId, database }),
            },
          ]
        : []),
      ...(flatTarget
        ? [
            {
              label: 'Nova tabela',
              icon: Table01Icon,
              onSelect: () =>
                openStructureDialog({
                  kind: 'createTable',
                  target: flatTarget,
                }),
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

  /** Um servidor e, aberto, a subárvore dele (databases, schemas…) */
  const pushServer = (server: Server) => {
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
          isExpanded &&
          databases &&
          databases.length > FILTER_FROM &&
          !filters.get(serverNodeId)
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
          {
            label: 'Mover para grupo…',
            icon: FolderTransferIcon,
            onSelect: () => openGroupDialog({ kind: 'move', server }),
          },
        ],
      },
    });

    if (!isExpanded) return;

    if (dbQuery?.isError) {
      rows.push({
        variant: 'error',
        id: `${serverNodeId}|err`,
        level: 1,
        message: dbQuery.error.message,
        error: dbQuery.error,
        onRetry: () => void dbQuery.refetch(),
        onEdit: () => onEditServer(server),
      });
      return;
    }

    // Mongo/Redis have no schema level — render tables directly (BACKEND.md §6.3)
    const hasSchemas =
      capabilitiesByServer.get(server.id)?.data?.hasSchemas ?? true;

    if (!databases) return;

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
              ? (catalogStatus?.data?.syncing ??
                  catalogStatus?.isFetching ??
                  false) ||
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
          error: structureQuery.error,
          onRetry: () => void structureQuery.refetch(),
          onEdit: () => onEditServer(server),
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
  };

  /**
   * Os níveis da subárvore de um servidor são fixos (servidor 0, database 1…);
   * dentro de um grupo tudo desce um nível. Desloca as linhas já empurradas.
   */
  const indentFrom = (start: number) => {
    for (let i = start; i < rows.length; i++) {
      const row = rows[i];
      if (row.variant === 'node') row.props.level += 1;
      else row.level += 1;
    }
  };

  /** Um grupo e, aberto, os servidores dele um nível abaixo */
  const pushGroup = (group: ServerGroup, members: Server[]) => {
    const nodeId = groupNodeId(group.id);
    const isOpen = !collapsedGroups.has(group.id);

    rows.push({
      variant: 'node',
      id: nodeId,
      props: {
        level: 0,
        kind: 'group',
        name: group.name,
        subLabel: formatCount(members.length),
        hasChildren: members.length > 0,
        isExpanded: isOpen,
        onClick: () => toggleNode(nodeId),
        actions: [
          {
            label: 'Novo servidor aqui',
            icon: Add01Icon,
            onSelect: () => onCreateServer(group.id),
          },
          {
            label: 'Renomear',
            icon: PencilEdit02Icon,
            onSelect: () => openGroupDialog({ kind: 'rename', group }),
          },
          {
            label: 'Excluir grupo',
            icon: Delete02Icon,
            danger: true,
            separated: true,
            onSelect: () =>
              openGroupDialog({
                kind: 'delete',
                group,
                serverCount: members.length,
              }),
          },
        ],
      },
    });

    if (!isOpen) return;
    const start = rows.length;
    for (const server of members) pushServer(server);
    indentFrom(start);
  };

  // Grupos primeiro (em ordem alfabética, como vêm do backend), depois os
  // servidores fora de grupo. Um `groupId` de grupo que não está na lista
  // (cache defasado) conta como fora de grupo, para o servidor não sumir.
  const groups = groupsQuery.data ?? [];
  const membersByGroup = new Map<number, Server[]>(
    groups.map(group => [group.id, []]),
  );
  const ungrouped: Server[] = [];
  for (const server of servers) {
    const members =
      server.groupId == null ? undefined : membersByGroup.get(server.groupId);
    if (members) members.push(server);
    else ungrouped.push(server);
  }
  for (const group of groups) pushGroup(group, membersByGroup.get(group.id)!);
  for (const server of ungrouped) pushServer(server);

  return {
    rows,
    isLoading: serversQuery.isLoading,
    isError: serversQuery.isError,
    error: serversQuery.error,
    retry: () => void serversQuery.refetch(),
    isEmpty: !serversQuery.data?.length && !groupsQuery.data?.length,
  };
};
