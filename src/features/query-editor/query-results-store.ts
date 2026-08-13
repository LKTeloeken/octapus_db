import { create } from 'zustand';
import type { QueryResult } from '@/api/types/query.types';

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

interface QueryResultsState {
  runs: Map<string, EditorRun>;
  setRun: (tabId: string, run: EditorRun) => void;
  appendRows: (tabId: string, page: QueryResult) => void;
  clearRun: (tabId: string) => void;
}

export const useQueryResultsStore = create<QueryResultsState>(set => ({
  runs: new Map(),

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
      if (!state.runs.has(tabId)) return state;
      const runs = new Map(state.runs);
      runs.delete(tabId);
      return { runs };
    });
  },
}));
