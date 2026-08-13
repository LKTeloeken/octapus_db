import { create } from 'zustand';
import type { ColumnFilter, SortSpec } from '@/api/types/browse.types';
import { encodeNodeId } from '@/lib/node-ref';

/**
 * Tabs hold only identity + intent. Data, loading and errors live in the
 * React Query layer (queries/) — never here.
 */
interface TabBase {
  id: string;
  title: string;
  serverId: number;
  database: string;
}

export interface QueryTab extends TabBase {
  kind: 'query';
  /** Editor text */
  content: string;
  /** Nomes das colunas ocultas no front (só renderização, não muda o query) */
  hiddenColumns: string[];
}

export interface BrowseTab extends TabBase {
  kind: 'browse';
  /** null for databases without schemas (Mongo/Redis) */
  schema: string | null;
  table: string;
  filters: ColumnFilter[];
  sort: SortSpec[];
  /** Nomes das colunas ocultas no front (só renderização, não muda o query) */
  hiddenColumns: string[];
}

export type WorkspaceTab = QueryTab | BrowseTab;

interface TabsState {
  tabs: Map<string, WorkspaceTab>;
  activeTabId: string | null;

  openQueryTab: (params: {
    serverId: number;
    database: string;
    title?: string;
    content?: string;
  }) => string;
  openBrowseTab: (params: {
    serverId: number;
    database: string;
    schema: string | null;
    table: string;
  }) => string;
  closeTab: (id: string) => void;
  setActiveTab: (id: string) => void;
  setQueryContent: (id: string, content: string) => void;
  setQueryHiddenColumns: (id: string, hiddenColumns: string[]) => void;
  setBrowseSort: (id: string, sort: SortSpec[]) => void;
  setBrowseFilters: (id: string, filters: ColumnFilter[]) => void;
  setBrowseHiddenColumns: (id: string, hiddenColumns: string[]) => void;
}

let queryTabCounter = 0;

function patchTab<T extends WorkspaceTab>(
  tabs: Map<string, WorkspaceTab>,
  id: string,
  kind: T['kind'],
  patch: Partial<T>,
): Map<string, WorkspaceTab> {
  const tab = tabs.get(id);
  if (!tab || tab.kind !== kind) return tabs;
  return new Map(tabs).set(id, { ...tab, ...patch });
}

export const useTabsStore = create<TabsState>((set, get) => ({
  tabs: new Map(),
  activeTabId: null,

  openQueryTab: ({ serverId, database, title, content }) => {
    const id = `query|${serverId}|${database}|${++queryTabCounter}|${Date.now()}`;
    const tab: QueryTab = {
      id,
      kind: 'query',
      title: title ?? database,
      serverId,
      database,
      content: content ?? '',
      hiddenColumns: [],
    };

    set(state => ({
      tabs: new Map(state.tabs).set(id, tab),
      activeTabId: id,
    }));

    return id;
  },

  openBrowseTab: ({ serverId, database, schema, table }) => {
    const id = encodeNodeId({
      serverId,
      database,
      schema: schema ?? undefined,
      table,
    });

    // One browse tab per table — re-opening just activates it
    if (get().tabs.has(id)) {
      set({ activeTabId: id });
      return id;
    }

    const tab: BrowseTab = {
      id,
      kind: 'browse',
      title: table,
      serverId,
      database,
      schema,
      table,
      filters: [],
      sort: [],
      hiddenColumns: [],
    };

    set(state => ({
      tabs: new Map(state.tabs).set(id, tab),
      activeTabId: id,
    }));

    return id;
  },

  closeTab: id => {
    set(state => {
      const order = Array.from(state.tabs.keys());
      const closedIndex = order.indexOf(id);
      if (closedIndex === -1) return state;

      const tabs = new Map(state.tabs);
      tabs.delete(id);

      let activeTabId = state.activeTabId;
      if (activeTabId === id) {
        const remaining = Array.from(tabs.keys());
        activeTabId =
          remaining[Math.min(closedIndex, remaining.length - 1)] ?? null;
      }

      return { tabs, activeTabId };
    });
  },

  setActiveTab: id => {
    if (get().tabs.has(id)) set({ activeTabId: id });
  },

  setQueryContent: (id, content) => {
    set(state => ({
      tabs: patchTab<QueryTab>(state.tabs, id, 'query', { content }),
    }));
  },

  setQueryHiddenColumns: (id, hiddenColumns) => {
    set(state => ({
      tabs: patchTab<QueryTab>(state.tabs, id, 'query', { hiddenColumns }),
    }));
  },

  setBrowseSort: (id, sort) => {
    set(state => ({
      tabs: patchTab<BrowseTab>(state.tabs, id, 'browse', { sort }),
    }));
  },

  setBrowseFilters: (id, filters) => {
    set(state => ({
      tabs: patchTab<BrowseTab>(state.tabs, id, 'browse', { filters }),
    }));
  },

  setBrowseHiddenColumns: (id, hiddenColumns) => {
    set(state => ({
      tabs: patchTab<BrowseTab>(state.tabs, id, 'browse', { hiddenColumns }),
    }));
  },
}));

export const useActiveTab = (): WorkspaceTab | null =>
  useTabsStore(state =>
    state.activeTabId ? (state.tabs.get(state.activeTabId) ?? null) : null,
  );
