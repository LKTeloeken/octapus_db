import { useCallback, useMemo, useState } from 'react';
import { useRunQuery } from '@/shared/hooks/use-run-query/use-run-query';
import { applyRowEdits, cancelQuery } from '@/api/database/database-methods';
import { useBuildQueries } from '../use-build-queries/use-build-queries';
import toast from 'react-hot-toast';

import { type Tab, type TabSort, TabType } from '@/shared/models/tabs.types';
import { TreeNodeType, type TreeNode } from '@/shared/models/database.types';
import type { HandleFetchStructure } from '../use-data-structure/use-data-structure.types';
import type { ExecuteQueryOptions } from '@/api/database/database-methods.types';
import type { RowEdit } from '@/api/database/database-responses.types';
import { useStore } from '@/stores';
import { ensureServerConnection } from '@/api/server/methods';

const DEFAULT_QUERY_OPTIONS: ExecuteQueryOptions = {
  unlimited: false,
  countTotal: true,
  limit: 500,
  offset: 0,
};

function createTab(
  id: string,
  serverId: number,
  databaseName: string,
  initialData?: Partial<Tab>,
): Tab {
  return {
    id,
    serverId,
    databaseName,
    title: databaseName,
    content: '',
    type: TabType.View,
    sort: null,
    loading: false,
    loadingMore: false,
    loadingChanges: false,
    queryOptions: DEFAULT_QUERY_OPTIONS,
    ...(initialData ?? {}),
  };
}

export function useTabs(loadDatabaseStructure: HandleFetchStructure) {
  const { runQuery } = useRunQuery();
  const { selectQuery } = useBuildQueries();
  const { fetchColumns, getStructure, recordRecentOpenedTable } = useStore();

  const [tabs, setTabs] = useState<Map<string, Tab>>(new Map());
  const [activeTabId, setActiveTabId] = useState<string | null>(null);

  const tabsList = useMemo(() => Array.from(tabs.values()), [tabs]);
  const activeTab = useMemo(
    () => (activeTabId ? (tabs.get(activeTabId) ?? null) : null),
    [tabs, activeTabId],
  );

  const updateTab = useCallback((id: string, patch: Partial<Tab>) => {
    setTabs(prev => {
      const existing = prev.get(id);
      if (!existing) return prev;
      return new Map(prev).set(id, { ...existing, ...patch });
    });
  }, []);

  const openTab = useCallback(
    (serverId: number, databaseName: string, initialData?: Partial<Tab>) => {
      const newTabId = `${serverId}-${databaseName}-${Date.now()}`;
      const targetTabId = initialData?.id ?? newTabId;
      const newTab = createTab(targetTabId, serverId, databaseName, initialData);

      setTabs(prev => new Map(prev).set(targetTabId, newTab));
      setActiveTabId(targetTabId);
      loadDatabaseStructure(serverId, databaseName);
      return targetTabId;
    },
    [loadDatabaseStructure],
  );

  const getDefaultSort = useCallback(
    async (
      serverId: number,
      databaseName: string,
      schemaName: string,
      tableName: string,
    ): Promise<TabSort | null> => {
      const structure = getStructure(serverId, databaseName);
      const currentTable = structure?.schemas
        .find(schema => schema.name === schemaName)
        ?.tables.find(table => table.name === tableName);

      const availableColumns =
        currentTable?.columns ??
        (await fetchColumns(serverId, databaseName, schemaName, tableName));

      const firstPrimaryKey = availableColumns.find(column => column.isPrimaryKey);
      return firstPrimaryKey
        ? { column: firstPrimaryKey.name, direction: 'desc' }
        : null;
    },
    [fetchColumns, getStructure],
  );

  const openTableByReference = useCallback(
    async (
      serverId: number,
      databaseName: string,
      schemaName: string,
      tableName: string,
    ) => {
      await ensureServerConnection(serverId, databaseName);
      const tableTabId = `${serverId}-${databaseName}-${schemaName}-${tableName}`;
      const recentKey = `${serverId}:${databaseName}:${schemaName}:${tableName}`;

      if (tabs.has(tableTabId)) {
        setActiveTabId(tableTabId);
        recordRecentOpenedTable({
          key: recentKey,
          serverId,
          serverName: `Server ${serverId}`,
          databaseName,
          schemaName,
          tableName,
        });
        return;
      }

      const defaultSort = await getDefaultSort(
        serverId,
        databaseName,
        schemaName,
        tableName,
      );
      const tableQuery = selectQuery(
        schemaName,
        tableName,
        defaultSort ? [defaultSort.column] : [],
        defaultSort?.direction ?? 'desc',
      );

      openTab(serverId, databaseName, {
        title: tableName,
        content: tableQuery,
        type: TabType.View,
        viewSchema: schemaName,
        viewTable: tableName,
        sort: defaultSort,
        id: tableTabId,
      });

      recordRecentOpenedTable({
        key: recentKey,
        serverId,
        serverName: `Server ${serverId}`,
        databaseName,
        schemaName,
        tableName,
      });
    },
    [getDefaultSort, openTab, selectQuery, tabs, recordRecentOpenedTable],
  );

  const openQueryTab = useCallback(
    (serverId: number, databaseName: string, title?: string, content = '') => {
      openTab(serverId, databaseName, {
        title: title ?? databaseName,
        content,
        type: TabType.Query,
      });
    },
    [openTab],
  );

  const openTableTab = useCallback(
    (node: TreeNode) => {
      if (
        node.type !== TreeNodeType.Table ||
        node.metadata.type !== TreeNodeType.Table ||
        !node.metadata.schemaName ||
        !node.metadata.tableName
      ) {
        return;
      }

      openTableByReference(
        node.metadata.serverId,
        node.metadata.databaseName,
        node.metadata.schemaName,
        node.metadata.tableName,
      ).catch(error => {
        toast.error(
          error instanceof Error ? error.message : 'Failed to open table tab',
        );
      });
    },
    [openTableByReference],
  );

  const closeTab = useCallback((id: string) => {
    setTabs(prev => {
      const next = new Map(prev);
      next.delete(id);
      return next;
    });

    setActiveTabId(prevActiveId => {
      if (prevActiveId !== id) return prevActiveId;
      return null;
    });
  }, []);

  const setTabContent = useCallback(
    (id: string, content: string) => {
      updateTab(id, { content });
    },
    [updateTab],
  );

  const setQueryTabOptions = useCallback(
    (id: string, options: ExecuteQueryOptions) => {
      updateTab(id, { queryOptions: options });
    },
    [updateTab],
  );

  const runQueryTab = useCallback(
    async (
      id: string,
      query: string,
      options?: ExecuteQueryOptions,
      fallback?: { serverId: number; databaseName: string },
    ) => {
      const tab = tabs.get(id);
      const serverId = tab?.serverId ?? fallback?.serverId;
      const databaseName = tab?.databaseName ?? fallback?.databaseName;
      if (!serverId || !databaseName) return;

      const resetOptions: ExecuteQueryOptions = {
        ...(options ?? tab?.queryOptions ?? DEFAULT_QUERY_OPTIONS),
        offset: 0,
        countTotal: true,
      };

      updateTab(id, {
        loading: true,
        runningQueryId: tab?.runningQueryId ?? id,
        queryOptions: resetOptions,
      });

      try {
        const result = await runQuery(
          serverId,
          databaseName,
          query,
          resetOptions,
        );

        updateTab(id, {
          loading: false,
          runningQueryId: result.queryId,
          lastExecutedQuery: query,
          result,
        });
      } catch (error) {
        const message =
          error instanceof Error ? error.message : 'An unknown error occurred';

        if (message.toLowerCase().includes('canceling statement')) {
          toast('Query canceled');
        } else {
          toast.error(message);
        }
      } finally {
        updateTab(id, { loading: false, runningQueryId: null });
      }
    },
    [tabs, runQuery, updateTab],
  );

  const sortTableTab = useCallback(
    async (id: string, column: string) => {
      const tab = tabs.get(id);
      if (!tab || tab.type !== TabType.View || !tab.viewSchema || !tab.viewTable) {
        return;
      }

      const currentDirection =
        tab.sort?.column === column ? tab.sort.direction : null;
      const nextDirection = currentDirection === 'desc' ? 'asc' : 'desc';
      const nextSort: TabSort = { column, direction: nextDirection };
      const nextQuery = selectQuery(
        tab.viewSchema,
        tab.viewTable,
        [column],
        nextDirection,
      );

      updateTab(id, {
        sort: nextSort,
        content: nextQuery,
        queryOptions: { ...DEFAULT_QUERY_OPTIONS },
      });

      await runQueryTab(id, nextQuery, { ...DEFAULT_QUERY_OPTIONS });
    },
    [runQueryTab, selectQuery, tabs, updateTab],
  );

  const cancelQueryTab = useCallback(
    async (id: string) => {
      const tab = tabs.get(id);
      if (!tab || !tab.loading) return;

      await cancelQuery(tab.serverId, tab.databaseName, tab.runningQueryId ?? id);
    },
    [tabs],
  );

  const handleNextPage = useCallback(
    async (id: string) => {
      const tab = tabs.get(id);
      if (!tab || !tab.result?.hasMore || tab.loadingMore) return;

      const newOptions: ExecuteQueryOptions = {
        ...tab.queryOptions,
        offset: tab.queryOptions.offset + tab.queryOptions.limit,
        countTotal: false,
      };

      updateTab(id, { loadingMore: true, queryOptions: newOptions });

      try {
        const result = await runQuery(
          tab.serverId,
          tab.databaseName,
          tab.lastExecutedQuery ?? '',
          newOptions,
        );

        updateTab(id, {
          loadingMore: false,
          result: {
            ...tab.result,
            rows: [...tab.result.rows, ...result.rows],
            hasMore: result.hasMore,
            rowCount: tab.result.rowCount + result.rowCount,
          },
        });
      } catch (error) {
        toast.error(
          error instanceof Error ? error.message : 'An unknown error occurred',
        );
        updateTab(id, { loadingMore: false });
      }
    },
    [tabs, runQuery, updateTab],
  );

  const applyQueryTabChanges = useCallback(
    async (id: string, edits: RowEdit[]) => {
      const tab = tabs.get(id);
      if (!tab || !tab.result?.editableInfo) return;

      updateTab(id, { loadingChanges: true });

      try {
        const result = await applyRowEdits(
          tab.serverId,
          tab.databaseName,
          tab.result.editableInfo,
          edits,
        );

        toast.success(`${result.affectedRows} rows updated`);

        await runQueryTab(id, tab.lastExecutedQuery ?? '', {
          ...DEFAULT_QUERY_OPTIONS,
        });
      } catch (error) {
        console.log('error', error);
        toast.error(
          error instanceof Error ? error.message : 'An unknown error occurred',
        );
      } finally {
        updateTab(id, { loadingChanges: false });
      }
    },
    [tabs, runQueryTab, updateTab],
  );

  const switchViewTabToQuery = useCallback(
    (id: string) => {
      const tab = tabs.get(id);
      if (!tab || tab.type !== TabType.View) return;

      const fallbackQuery =
        tab.viewSchema && tab.viewTable
          ? selectQuery(tab.viewSchema, tab.viewTable)
          : tab.content;

      updateTab(id, {
        type: TabType.Query,
        content: tab.content?.trim() ? tab.content : fallbackQuery,
      });
    },
    [selectQuery, tabs, updateTab],
  );

  return {
    tabs: tabsList,
    activeTab,
    activeTabId,
    openTab,
    openTableTab,
    openTableByReference,
    openQueryTab,
    switchViewTabToQuery,
    closeTab,
    setActiveTabId,
    setTabContent,
    setQueryTabOptions,
    runQueryTab,
    sortTableTab,
    cancelQueryTab,
    handleNextPage,
    applyQueryTabChanges,
  };
}

export default useTabs;
