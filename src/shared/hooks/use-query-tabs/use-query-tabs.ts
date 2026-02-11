import { useCallback, useMemo, useState } from 'react';
import { useRunQuery } from '@/shared/hooks/use-run-query/use-run-query';
import toast from 'react-hot-toast';

import type { QueryTab } from '@/shared/models/query-tabs.types';
import type { ExecuteQueryOptions } from '@/api/database/database-methods.types';

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
  ) => {
    const tab = queryTabs.get(id);
    if (!tab) return;

    handleSetTabQuery(id, { loading: true });

    try {
      const result = await runQuery(
        tab.serverId,
        tab.databaseName,
        query,
        options || tab.queryOptions,
      );

      console.log('result', result);

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

  const handleNextPage = (id: string) => {
    const tab = queryTabs.get(id);
    if (!tab) return;

    const newOptions: ExecuteQueryOptions = {
      ...tab.queryOptions,
      offset: tab.queryOptions.offset + tab.queryOptions.limit,
    };

    handleSetTabQuery(id, { queryOptions: newOptions });

    runQueryTab(id, tab.lastExecutedQuery || '', newOptions);
  };

  return {
    queryTabs,
    addQueryTab,
    closeQueryTab,
    setQueryTabOptions,
    runQueryTab,
    handleNextPage,
  };
}
