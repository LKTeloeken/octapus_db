import { useCallback, useMemo, useState } from 'react';
import { useRunQuery } from '@/shared/hooks/use-run-query/use-run-query';
import { applyRowEdits } from '@/api/database/database-methods';
import toast from 'react-hot-toast';

import type { ApplyQueryTabChanges } from './use-query-tabs.types';
import type { QueryTab } from '@/shared/models/query-tabs.types';
import type { ExecuteQueryOptions } from '@/api/database/database-methods.types';
import type { RowEdit } from '@/api/database/database-responses.types';

export function useQueryTabs() {
  const { runQuery, loading: loadingQuery } = useRunQuery();
  const [queryTabs, setQueryTabs] = useState<Map<string, QueryTab>>(new Map());

  const defaultQueryOptions: ExecuteQueryOptions = useMemo(
    () => ({
      unlimited: false,
      countTotal: true,
      limit: 500,
      offset: 0,
    }),
    [],
  );

  const handleSetTabQuery = useCallback(
    (id: string, content: Partial<QueryTab>) => {
      setQueryTabs(prev => {
        const existingTab = prev.get(id);

        if (!existingTab) {
          if (!content.serverId || !content.databaseName) return prev;

          const newTab: QueryTab = {
            serverId: content.serverId,
            databaseName: content.databaseName,
            loading: false,
            loadingMore: false,
            loadingChanges: false,
            queryOptions: defaultQueryOptions,
          };

          return new Map(prev).set(id, newTab);
        }

        return new Map(prev).set(id, { ...(existingTab || {}), ...content });
      });
    },
    [],
  );

  const addQueryTab = (id: string, serverId: number, databaseName: string) => {
    handleSetTabQuery(id, { serverId, databaseName });
  };

  const closeQueryTab = (id: string) => {
    setQueryTabs(prev => {
      const newTabs = new Map(prev);
      newTabs.delete(id);
      return newTabs;
    });
  };

  const setQueryTabOptions = (id: string, options: ExecuteQueryOptions) => {
    handleSetTabQuery(id, { queryOptions: options });
  };

  const runQueryTab = async (
    id: string,
    query: string,
    options?: ExecuteQueryOptions,
    fallback?: { serverId: number; databaseName: string },
  ) => {
    const tab = queryTabs.get(id);
    const serverId = tab?.serverId ?? fallback?.serverId;
    const databaseName = tab?.databaseName ?? fallback?.databaseName;
    if (!serverId || !databaseName) return;

    const resetOptions: ExecuteQueryOptions = {
      ...(options || tab?.queryOptions || defaultQueryOptions),
      offset: 0,
      countTotal: true,
    };

    handleSetTabQuery(id, {
      loading: true,
      queryOptions: resetOptions,
      serverId,
      databaseName,
    });

    try {
      const result = await runQuery(
        serverId,
        databaseName,
        query,
        resetOptions,
      );

      handleSetTabQuery(id, {
        loading: false,
        lastExecutedQuery: query,
        result,
      });
    } catch (error) {
      toast.error(
        error instanceof Error ? error.message : 'An unknown error occurred',
      );
    } finally {
      handleSetTabQuery(id, { loading: false });
    }
  };

  const handleNextPage = async (id: string) => {
    const tab = queryTabs.get(id);
    if (!tab || !tab.result?.hasMore || tab.loadingMore) return;

    const newOptions: ExecuteQueryOptions = {
      ...tab.queryOptions,
      offset: tab.queryOptions.offset + tab.queryOptions.limit,
      countTotal: false,
    };

    handleSetTabQuery(id, { loadingMore: true, queryOptions: newOptions });

    try {
      const result = await runQuery(
        tab.serverId,
        tab.databaseName,
        tab.lastExecutedQuery || '',
        newOptions,
      );

      handleSetTabQuery(id, {
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
      handleSetTabQuery(id, { loadingMore: false });
    }
  };

  const applyQueryTabChanges: ApplyQueryTabChanges = async (
    id: string,
    edits: RowEdit[],
  ) => {
    const tab = queryTabs.get(id);
    if (!tab || !tab.result?.editableInfo) return;

    handleSetTabQuery(id, { loadingChanges: true });

    try {
      const result = await applyRowEdits(
        tab.serverId,
        tab.databaseName,
        tab.result?.editableInfo,
        edits,
      );

      toast.success(`${result.affectedRows} rows updated`);

      await runQueryTab(id, tab.lastExecutedQuery || '', {
        offset: 0,
        countTotal: true,
        limit: 500,
        unlimited: false,
      });
    } catch (error) {
      console.log('error', error);
      toast.error(
        error instanceof Error ? error.message : 'An unknown error occurred',
      );
    } finally {
      handleSetTabQuery(id, { loadingChanges: false });
    }
  };

  return {
    queryTabs,
    addQueryTab,
    closeQueryTab,
    setQueryTabOptions,
    runQueryTab,
    handleNextPage,
    applyQueryTabChanges,
  };
}
