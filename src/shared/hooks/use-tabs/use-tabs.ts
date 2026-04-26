import { useCallback, useMemo, useState } from 'react';
import { useRunQuery } from '@/shared/hooks/use-run-query/use-run-query';
import { applyRowEdits } from '@/api/database/database-methods';
import { useBuildQueries } from '../use-build-queries/use-build-queries';
import toast from 'react-hot-toast';

import { type Tab, TabType } from '@/shared/models/tabs.types';
import { TreeNodeType, type TreeNode } from '@/shared/models/database.types';
import type { HandleFetchStructure } from '../use-data-structure/use-data-structure.types';
import type { ExecuteQueryOptions } from '@/api/database/database-methods.types';
import type { RowEdit } from '@/api/database/database-responses.types';

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
    type: TabType.Query,
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
      console.log('openTab', serverId, databaseName, initialData);
      const newTabId = `${serverId}-${databaseName}-${Date.now()}`;
      const newTab = createTab(newTabId, serverId, databaseName, initialData);

      setTabs(prev => new Map(prev).set(initialData?.id ?? newTabId, newTab));
      setActiveTabId(newTabId);
      loadDatabaseStructure(serverId, databaseName);
    },
    [loadDatabaseStructure],
  );

  const openTableTab = useCallback(
    (node: TreeNode) => {
      if (node.type !== TreeNodeType.Table) return;

      const [, tableName, schemaName, databaseName, serverId] =
        node.id.split('-');
      const tableTabId = `${serverId}-${databaseName}-${tableName}`;

      // If the tab already exists, set it as active and run the query to refresh the data
      if (tabs.has(tableTabId)) {
        const tab = tabs.get(tableTabId);
        if (!tab) return;

        setActiveTabId(tableTabId);

        runQueryTab(tableTabId, tab.content, undefined, {
          serverId: Number(serverId),
          databaseName,
        });
        return;
      }

      // If the tab does not exist, create it and run the query to fetch the data
      const tableQuery = selectQuery(schemaName, tableName);

      openTab(Number(serverId), databaseName, {
        title: tableName,
        content: tableQuery,
        type: TabType.View,
        id: tableTabId,
      });

      setActiveTabId(tableTabId);
    },
    [openTab, selectQuery],
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
      // if (!query) return;

      const tab = tabs.get(id);
      const serverId = tab?.serverId ?? fallback?.serverId;
      const databaseName = tab?.databaseName ?? fallback?.databaseName;
      if (!serverId || !databaseName) return;

      const resetOptions: ExecuteQueryOptions = {
        ...(options ?? tab?.queryOptions ?? DEFAULT_QUERY_OPTIONS),
        offset: 0,
        countTotal: true,
      };

      updateTab(id, { loading: true, queryOptions: resetOptions });

      try {
        const result = await runQuery(
          serverId,
          databaseName,
          query,
          resetOptions,
        );

        updateTab(id, {
          loading: false,
          lastExecutedQuery: query,
          result,
        });
      } catch (error) {
        console.log('runQueryTab', error);
        toast.error(
          error instanceof Error ? error.message : 'An unknown error occurred',
        );
      } finally {
        updateTab(id, { loading: false });
      }
    },
    [tabs, runQuery, updateTab],
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

  return {
    tabs: tabsList,
    activeTab,
    activeTabId,
    openTab,
    openTableTab,
    closeTab,
    setActiveTabId,
    setTabContent,
    setQueryTabOptions,
    runQueryTab,
    handleNextPage,
    applyQueryTabChanges,
  };
}

export default useTabs;
