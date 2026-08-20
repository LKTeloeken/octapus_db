import { create } from 'zustand';
import type { QueryMessage, QueryResult } from '@/api/types/query.types';

/**
 * Ephemeral per-tab editor results. Free-editor runs are snapshots, not
 * cacheable server-state — so they live here, keyed by tab id, and survive
 * tab switches. Cleared when the tab closes.
 */
export interface EditorRun {
  /** The query that produced the result (used by load-more and refresh) */
  query: string;
  result: QueryResult;
}

/** Um bloco do log: uma execução e tudo que o banco disse durante ela. */
export interface QueryLogEntry {
  id: string;
  startedAt: number;
  query: string;
  messages: QueryMessage[];
}

export type BottomTab = 'results' | 'messages';

/**
 * Tetos de memória: o log vive enquanto a aba estiver aberta.
 *
 * O corte de verdade é no backend (MAX_STREAMED_NOTICES), que para de
 * encaminhar e avisa no próprio log; o teto por entrada aqui é só rede de
 * segurança, e por isso fica acima do de lá.
 */
const MAX_ENTRIES_PER_TAB = 100;
const MAX_MESSAGES_PER_ENTRY = 5000;

const countMessages = (entries: QueryLogEntry[]) =>
  entries.reduce((total, entry) => total + entry.messages.length, 0);

interface QueryResultsState {
  runs: Map<string, EditorRun>;
  logs: Map<string, QueryLogEntry[]>;
  /** Aba ativa da área inferior; mora aqui porque o painel desmonta ao trocar de aba */
  bottomTabs: Map<string, BottomTab>;
  /** Quantas mensagens o usuário já viu por aba — a diferença vira o badge */
  seenCounts: Map<string, number>;

  setRun: (tabId: string, run: EditorRun) => void;
  appendRows: (tabId: string, page: QueryResult) => void;
  clearRun: (tabId: string) => void;

  startLogEntry: (tabId: string, query: string) => string;
  appendMessages: (
    tabId: string,
    entryId: string,
    messages: QueryMessage[],
  ) => void;
  clearLog: (tabId: string) => void;

  setBottomTab: (tabId: string, tab: BottomTab) => void;
}

export const useQueryResultsStore = create<QueryResultsState>(set => ({
  runs: new Map(),
  logs: new Map(),
  bottomTabs: new Map(),
  seenCounts: new Map(),

  setRun: (tabId, run) => {
    set(state => ({ runs: new Map(state.runs).set(tabId, run) }));
  },

  appendRows: (tabId, page) => {
    set(state => {
      const existing = state.runs.get(tabId);
      if (!existing) return state;

      const merged: EditorRun = {
        ...existing,
        result: {
          ...existing.result,
          rows: [...existing.result.rows, ...page.rows],
          rowCount: existing.result.rowCount + page.rowCount,
          hasMore: page.hasMore,
        },
      };

      return { runs: new Map(state.runs).set(tabId, merged) };
    });
  },

  clearRun: tabId => {
    set(state => {
      const hasState =
        state.runs.has(tabId) ||
        state.logs.has(tabId) ||
        state.bottomTabs.has(tabId);
      if (!hasState) return state;

      const runs = new Map(state.runs);
      const logs = new Map(state.logs);
      const bottomTabs = new Map(state.bottomTabs);
      const seenCounts = new Map(state.seenCounts);
      runs.delete(tabId);
      logs.delete(tabId);
      bottomTabs.delete(tabId);
      seenCounts.delete(tabId);

      return { runs, logs, bottomTabs, seenCounts };
    });
  },

  startLogEntry: (tabId, query) => {
    const id = `${Date.now()}-${Math.random().toString(36).slice(2, 9)}`;

    set(state => {
      const entries = state.logs.get(tabId) ?? [];
      const next = [
        ...entries,
        { id, startedAt: Date.now(), query, messages: [] },
      ].slice(-MAX_ENTRIES_PER_TAB);

      return { logs: new Map(state.logs).set(tabId, next) };
    });

    return id;
  },

  appendMessages: (tabId, entryId, messages) => {
    if (messages.length === 0) return;

    set(state => {
      const entries = state.logs.get(tabId);
      if (!entries) return state;

      const index = entries.findIndex(entry => entry.id === entryId);
      // A entrada some quando o log é limpo no meio de uma execução; as
      // mensagens que ainda estavam a caminho são descartadas.
      if (index === -1) return state;

      const target = entries[index];
      const next = [...entries];
      next[index] = {
        ...target,
        messages: [...target.messages, ...messages].slice(
          -MAX_MESSAGES_PER_ENTRY,
        ),
      };

      const logs = new Map(state.logs).set(tabId, next);

      // Chegando com o log aberto, já nasce lida — o badge não deve piscar
      // enquanto o usuário está olhando para as mensagens.
      if (state.bottomTabs.get(tabId) !== 'messages') {
        return { logs };
      }

      return {
        logs,
        seenCounts: new Map(state.seenCounts).set(tabId, countMessages(next)),
      };
    });
  },

  clearLog: tabId => {
    set(state => {
      if (!state.logs.has(tabId)) return state;

      const logs = new Map(state.logs);
      const seenCounts = new Map(state.seenCounts);
      logs.delete(tabId);
      seenCounts.delete(tabId);

      return { logs, seenCounts };
    });
  },

  setBottomTab: (tabId, tab) => {
    set(state => {
      const bottomTabs = new Map(state.bottomTabs).set(tabId, tab);
      if (tab !== 'messages') return { bottomTabs };

      // Abrir o log marca tudo como lido.
      const total = countMessages(state.logs.get(tabId) ?? []);
      return {
        bottomTabs,
        seenCounts: new Map(state.seenCounts).set(tabId, total),
      };
    });
  },
}));
