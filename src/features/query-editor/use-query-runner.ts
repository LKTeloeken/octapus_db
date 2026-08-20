import { useCallback, useMemo, useState } from 'react';
import toast from 'react-hot-toast';
import type { QueryMessage } from '@/api/types/query.types';
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
import {
  useQueryResultsStore,
  type BottomTab,
  type QueryLogEntry,
} from './query-results-store';
import { useSqlCompletion } from './use-sql-completion';

const PLACEHOLDERS: Record<string, string> = {
  postgres: 'SELECT * FROM ...',
  mongodb: "db.collection.find({ ... })",
  redis: 'GET user:1 · HGETALL session:abc · SCAN 0 MATCH user:*',
};

/** Identidade estável para abas sem log, evitando re-render por referência nova */
const EMPTY_LOG: QueryLogEntry[] = [];

export const useQueryRunner = (tab: QueryTab) => {
  const setQueryContent = useTabsStore(state => state.setQueryContent);
  const run = useQueryResultsStore(state => state.runs.get(tab.id) ?? null);
  const setRun = useQueryResultsStore(state => state.setRun);
  const appendRows = useQueryResultsStore(state => state.appendRows);

  const log = useQueryResultsStore(state => state.logs.get(tab.id));
  const seenCount = useQueryResultsStore(
    state => state.seenCounts.get(tab.id) ?? 0,
  );
  const bottomTab = useQueryResultsStore(
    state => state.bottomTabs.get(tab.id) ?? 'results',
  );
  const startLogEntry = useQueryResultsStore(state => state.startLogEntry);
  const appendMessages = useQueryResultsStore(state => state.appendMessages);
  const clearLogInStore = useQueryResultsStore(state => state.clearLog);
  const setBottomTabInStore = useQueryResultsStore(state => state.setBottomTab);

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

      const entryId = startLogEntry(tab.id, query);

      // Um RAISE dentro de um loop dispara centenas de mensagens. Sem esse
      // buffer seria um set() do Zustand — e um re-render — por mensagem.
      let pending: QueryMessage[] = [];
      let frame: number | null = null;
      let sawDbError = false;

      const flush = () => {
        frame = null;
        if (pending.length === 0) return;

        const batch = pending;
        pending = [];
        appendMessages(tab.id, entryId, batch);
      };

      const onMessage = (message: QueryMessage) => {
        if (message.kind === 'error') sawDbError = true;
        pending.push(message);
        frame ??= requestAnimationFrame(flush);
      };

      try {
        const result = await executeQuery.mutateAsync({
          serverId: tab.serverId,
          database: tab.database,
          query,
          onMessage,
        });
        setRun(tab.id, { query, result });
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        toast.error(message);

        // Falha fora do banco (conexão, adapter) não passa pelo canal, então
        // não teria registro nenhum no log.
        if (!sawDbError) {
          pending.push({
            kind: 'error',
            severity: 'ERROR',
            message,
            detail: null,
            hint: null,
            context: null,
            sqlState: null,
            position: null,
            timestampMs: Date.now(),
          });
        }

        // O grid segue mostrando o resultado antigo depois de uma falha; a
        // informação útil está no log.
        setBottomTabInStore(tab.id, 'messages');
      } finally {
        if (frame !== null) cancelAnimationFrame(frame);
        flush();
      }
    },
    [
      tab.id,
      tab.serverId,
      tab.database,
      executeQuery,
      setRun,
      startLogEntry,
      appendMessages,
      setBottomTabInStore,
    ],
  );

  const loadMore = useCallback(async () => {
    if (!run || !run.result.hasMore || isLoadingMore) return;

    setIsLoadingMore(true);
    try {
      // Sem onMessage de propósito: carregar mais reexecuta a mesma query e
      // reemitiria todos os RAISE, duplicando o log sem valor nenhum.
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

  const clearLog = useCallback(
    () => clearLogInStore(tab.id),
    [tab.id, clearLogInStore],
  );

  const setBottomTab = useCallback(
    (next: BottomTab) => setBottomTabInStore(tab.id, next),
    [tab.id, setBottomTabInStore],
  );

  const unreadMessages = useMemo(() => {
    const total = (log ?? []).reduce(
      (count, entry) => count + entry.messages.length,
      0,
    );
    return Math.max(0, total - seenCount);
  }, [log, seenCount]);

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
    log: log ?? EMPTY_LOG,
    unreadMessages,
    clearLog,
    bottomTab,
    setBottomTab,
  };
};
