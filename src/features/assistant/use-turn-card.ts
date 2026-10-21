import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { useCallback, useEffect, useState } from 'react';
import toast from 'react-hot-toast';
import type { QueryLanguage } from '@/api/types/capabilities.types';
import type { QueryResult } from '@/api/types/query.types';
import {
  editorText,
  formatQueryText,
  relationNouns,
} from '@/lib/assistant/query-text';
import { useExecuteAssistantQuery } from '@/queries/use-assistant-conversations';
import {
  useAssistantStore,
  type AssistantTarget,
} from '@/stores/assistant-store';
import { useTabsStore } from '@/stores/tabs-store';
import type { ConversationTurn } from './use-conversation-view';

/** Linhas por página do resultado (o backend corta em 1.000) */
export const RESULT_PAGE_SIZE = 200;

const messageOf = (error: unknown) =>
  error instanceof Error ? error.message : String(error);

/**
 * Como a consulta da resposta está, para o selo e as ações. Nunca "executada"
 * sem o resultado ou o registro do outcome (RN-21).
 */
export type QueryState =
  | 'executed'
  /** Rodou dentro do turno (histórico): o resultado não fica guardado */
  | 'executed-earlier'
  | 'executing'
  /** Modo de revisão: espera o Executar (RN-13) */
  | 'review'
  | 'failed'
  | 'not-executed';

export const useTurnCard = ({
  turn,
  conversationId,
  target,
  flat,
  queryLanguage,
}: {
  turn: ConversationTurn;
  conversationId: string | null;
  target: AssistantTarget | null;
  flat: boolean;
  /** Linguagem do banco (capabilities): no Mongo, a operação em JSON */
  queryLanguage: QueryLanguage | null;
}) => {
  const { view, schemas } = turn;
  const attemptId = view.attemptId;
  const run = useAssistantStore(state =>
    attemptId ? state.runs[attemptId] : undefined,
  );
  const patchRun = useAssistantStore(state => state.patchRun);
  const openQueryTab = useTabsStore(state => state.openQueryTab);
  const executeQuery = useExecuteAssistantQuery();

  // Indentada para ler (no Mongo, o JSON da operação); é esta que roda
  const generated = formatQueryText(queryLanguage, view.output?.query ?? '');
  // A consulta da revisão é editável; começa como o modelo gerou
  const [draft, setDraft] = useState(generated);
  useEffect(() => {
    if (!view.streaming) setDraft(generated);
  }, [generated, view.streaming]);

  const isQuery = view.output?.kind === 'query';
  const searchPath = flat ? null : (schemas[0] ?? null);

  let queryState: QueryState = 'not-executed';
  if (run?.result) queryState = 'executed';
  else if (run?.executing) queryState = 'executing';
  else if (view.executedInTurn) queryState = 'executed-earlier';
  else if (view.outcome === 'running' && view.phase === 'executing') {
    queryState = 'executing';
  } else if (
    view.failure ||
    view.outcome === 'failed' ||
    view.outcome === 'exhausted'
  ) {
    queryState = 'failed';
  } else if (isQuery && view.outcome === 'answered') queryState = 'review';

  const runQuery = useCallback(
    async (query: string) => {
      if (!target || !attemptId || !query.trim()) return;
      const userEdited = query !== generated;
      patchRun(attemptId, { executing: true, error: null });
      try {
        const result = await executeQuery.mutateAsync({
          serverId: target.serverId,
          database: target.database,
          query,
          page: {
            limit: RESULT_PAGE_SIZE,
            offset: 0,
            countTotal: false,
            schema: searchPath,
          },
          // Só a revisão desta sessão completa o outcome da tentativa: o
          // plano chegou ao vivo e a execução automática não rodou
          attemptId: run?.plan && !view.executedInTurn ? attemptId : null,
          userEdited,
        });
        patchRun(attemptId, {
          executing: false,
          result,
          query,
          userEdited,
        });
      } catch (error) {
        patchRun(attemptId, { executing: false, error: messageOf(error) });
      }
    },
    [
      target,
      attemptId,
      generated,
      patchRun,
      executeQuery,
      searchPath,
      run?.plan,
      view.executedInTurn,
    ],
  );

  const loadMore = useCallback(async () => {
    if (!run?.result || !target || !attemptId) return;
    const current = run.result;
    if (!current.hasMore || run.loadingMore) return;
    patchRun(attemptId, { loadingMore: true });
    try {
      const page = await executeQuery.mutateAsync({
        serverId: target.serverId,
        database: target.database,
        query: run.query ?? generated,
        page: {
          limit: RESULT_PAGE_SIZE,
          offset: current.rows.length,
          countTotal: false,
          schema: searchPath,
        },
        userEdited: run.userEdited,
      });
      const merged: QueryResult = {
        ...current,
        rows: [...current.rows, ...page.rows],
        rowCount: current.rowCount + page.rowCount,
        hasMore: page.hasMore,
        executionTimeMs: page.executionTimeMs,
      };
      patchRun(attemptId, { loadingMore: false, result: merged });
    } catch (error) {
      patchRun(attemptId, { loadingMore: false });
      toast.error(messageOf(error));
    }
  }, [run, target, attemptId, patchRun, executeQuery, generated, searchPath]);

  /** C-10: a consulta (ou o código) numa aba de query, para ajustar e rodar */
  const openInEditor = useCallback(
    (text: string) => {
      if (!target || !text.trim()) return;
      openQueryTab({
        serverId: target.serverId,
        database: target.database,
        schema: searchPath,
        title: 'Assistente',
        // No Mongo, o comando que o editor roda (`db.<coleção>.find(…)`)
        content: editorText(queryLanguage, text),
      });
    },
    [openQueryTab, target, searchPath, queryLanguage],
  );

  const copy = useCallback(async (text: string) => {
    try {
      await writeText(text);
      toast.success('Copiado.');
    } catch (error) {
      toast.error(messageOf(error));
    }
  }, []);

  // Avalia-se a resposta que está na tela, com o turno terminado e já no
  // histórico do servidor (a API recusa turno sem tentativa concluída)
  const feedbackTarget =
    conversationId &&
    view.turnId &&
    turn.inHistory &&
    view.output &&
    !view.streaming &&
    view.outcome !== 'running'
      ? { conversationId, turnId: view.turnId }
      : null;

  return {
    view,
    feedbackTarget,
    run: run ?? null,
    queryState,
    isQuery,
    draft,
    setDraft,
    edited: draft !== generated,
    canExecute: !!target && !!attemptId && !run?.executing,
    runQuery,
    loadMore,
    openInEditor,
    copy,
    canOpenInEditor: !!target,
    nouns: relationNouns(queryLanguage),
  };
};
