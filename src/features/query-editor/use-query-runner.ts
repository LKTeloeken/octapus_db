import { useCallback, useState } from 'react';
import toast from 'react-hot-toast';
import type { SaveRowChanges } from '@/components/results-table/results-table.types';
import {
  useApplyRowEdits,
  useDeleteRows,
  useInsertRows,
} from '@/queries/use-apply-row-edits';
import { useCapabilities } from '@/queries/use-capabilities';
import { useExecuteQuery } from '@/queries/use-execute-query';
import { useServers } from '@/queries/use-servers';
import { useTabsStore, type QueryTab } from '@/stores/tabs-store';
import { useQueryResultsStore } from './query-results-store';
import { useSqlCompletion } from './use-sql-completion';

const PLACEHOLDERS: Record<string, string> = {
  postgres: 'SELECT * FROM ...',
  mongodb: "db.collection.find({ ... })",
  redis: 'GET user:1 · HGETALL session:abc · SCAN 0 MATCH user:*',
};

export const useQueryRunner = (tab: QueryTab) => {
  const setQueryContent = useTabsStore(state => state.setQueryContent);
  const run = useQueryResultsStore(state => state.runs.get(tab.id) ?? null);
  const setRun = useQueryResultsStore(state => state.setRun);
  const appendRows = useQueryResultsStore(state => state.appendRows);

  const executeQuery = useExecuteQuery();
  const applyEditsMutation = useApplyRowEdits();
  const insertRowsMutation = useInsertRows();
  const deleteRowsMutation = useDeleteRows();
  const [isLoadingMore, setIsLoadingMore] = useState(false);

  const { data: servers } = useServers();
  const { data: capabilities } = useCapabilities(tab.serverId);
  const supportsSql = capabilities?.supportsSql ?? true;

  const sqlCompletion = useSqlCompletion({
    serverId: tab.serverId,
    database: tab.database,
    enabled: supportsSql,
  });

  const dbType = servers?.find(s => s.id === tab.serverId)?.dbType ?? 'postgres';

  const executeRun = useCallback(
    async (query: string) => {
      if (!query.trim()) return;

      try {
        const result = await executeQuery.mutateAsync({
          serverId: tab.serverId,
          database: tab.database,
          query,
        });
        setRun(tab.id, { query, result });
      } catch (error) {
        toast.error(error instanceof Error ? error.message : String(error));
      }
    },
    [tab.id, tab.serverId, tab.database, executeQuery, setRun],
  );

  const loadMore = useCallback(async () => {
    if (!run || !run.result.hasMore || isLoadingMore) return;

    setIsLoadingMore(true);
    try {
      const page = await executeQuery.mutateAsync({
        serverId: tab.serverId,
        database: tab.database,
        query: run.query,
        options: { offset: run.result.rows.length, countTotal: false },
      });
      appendRows(tab.id, page);
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error));
    } finally {
      setIsLoadingMore(false);
    }
  }, [run, isLoadingMore, tab.id, tab.serverId, tab.database, executeQuery, appendRows]);

  const save = useCallback(
    async ({ edits, inserts, deletes }: SaveRowChanges) => {
      const editable = run?.result.editableInfo;
      if (!editable) return;

      const base = { serverId: tab.serverId, database: tab.database, editable };

      try {
        let affected = 0;
        if (deletes.length > 0) {
          const r = await deleteRowsMutation.mutateAsync({
            ...base,
            pkValues: deletes,
          });
          affected += r.affectedRows;
        }
        if (inserts.length > 0) {
          const r = await insertRowsMutation.mutateAsync({
            ...base,
            rows: inserts,
          });
          affected += r.affectedRows;
        }
        if (edits.length > 0) {
          const r = await applyEditsMutation.mutateAsync({ ...base, edits });
          affected += r.affectedRows;
        }
        toast.success(`${affected} linhas alteradas`);
        await executeRun(run.query);
      } catch (error) {
        toast.error(error instanceof Error ? error.message : String(error));
        throw error; // let ResultsTable keep the pending changes for retry
      }
    },
    [
      run,
      tab.serverId,
      tab.database,
      applyEditsMutation,
      insertRowsMutation,
      deleteRowsMutation,
      executeRun,
    ],
  );

  // Identidade estável: o `useCodeMirror` reconfigura o editor quando o `onChange` muda,
  // e isso aconteceria a cada tecla.
  const setContent = useCallback(
    (content: string) => setQueryContent(tab.id, content),
    [tab.id, setQueryContent],
  );

  return {
    result: run?.result ?? null,
    isRunning: executeQuery.isPending && !isLoadingMore,
    isLoadingMore,
    supportsSql,
    placeholder: PLACEHOLDERS[dbType] ?? PLACEHOLDERS.postgres,
    sqlCompletion,
    setContent,
    executeRun,
    loadMore,
    save,
  };
};
