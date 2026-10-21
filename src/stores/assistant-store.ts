import { create } from 'zustand';
import type { ReadOnlyPlan } from '@/api/types/assistant.types';
import type { QueryResult } from '@/api/types/query.types';
import type { LiveTurn, TurnContext } from '@/lib/assistant/turn-reducer';

/**
 * Estado de tela do assistente: o painel, as abas de conversa, o turno ao vivo
 * de cada aba e os resultados executados.
 *
 * **Só em memória, nunca persistido** (RN-01). Fica fora do `tabs-store` de
 * propósito: o `tabs-session` grava aquele store em disco. O histórico volta
 * do servidor (C-15) — reabrir o app começa com o painel vazio.
 */

/** Onde a conversa pergunta: servidor e database conectados (C-01, RN-17) */
export interface AssistantTarget {
  serverId: number;
  database: string;
  /** Schema de onde a conversa abriu (nó da árvore, aba); padrão das menções */
  schema: string | null;
}

export interface ConversationTab {
  /** Id local da aba (não é o da conversa) */
  id: string;
  target: AssistantTarget | null;
  /** `null` até a primeira pergunta: a conversa só nasce no servidor então */
  conversationId: string | null;
  /** Modelo escolhido antes de criar a conversa (modo `free`) */
  modelId: string | null;
  /** Rascunho da pergunta */
  draft: string;
}

/** O que foi executado de uma tentativa: plano e páginas do resultado */
export interface AttemptRun {
  plan: ReadOnlyPlan | null;
  result: QueryResult | null;
  /** A consulta que gerou o `result` (pode ser a editada na revisão) */
  query: string | null;
  userEdited: boolean;
  executing: boolean;
  loadingMore: boolean;
  /** Erro da execução manual, como o banco respondeu */
  error: string | null;
}

const EMPTY_RUN: AttemptRun = {
  plan: null,
  result: null,
  query: null,
  userEdited: false,
  executing: false,
  loadingMore: false,
  error: null,
};

interface AssistantState {
  panelOpen: boolean;
  tabs: ConversationTab[];
  activeTabId: string | null;
  /** Turno em andamento (ou recém-terminado) de cada aba */
  live: Record<string, LiveTurn>;
  /** Por `attemptId` */
  runs: Record<string, AttemptRun>;
  /**
   * O contexto de cada turno desta sessão (recorte da RN-26, menções que não
   * existem ou foram encurtadas), pelo `turnId` — a API não guarda isso
   */
  turnContexts: Record<string, TurnContext>;
  /** Pedido de foco no campo da pergunta (abrir pelo atalho, nova aba) */
  focusRequest: number;

  /**
   * Abre o painel. Com `target`, a conversa ativa passa a ele se ainda não
   * perguntou nada; senão abre uma aba nova para ele.
   */
  openPanel: (target?: AssistantTarget | null) => void;
  closePanel: () => void;
  togglePanel: () => void;
  /** Nova aba; sem `target`, herda o da aba ativa */
  newTab: (target?: AssistantTarget | null) => string;
  closeTab: (id: string) => void;
  setActiveTab: (id: string) => void;
  setTarget: (id: string, target: AssistantTarget | null) => void;
  setConversation: (id: string, conversationId: string | null) => void;
  /** Reabre uma conversa do histórico (ativa a aba se já estiver aberta) */
  openConversation: (
    conversationId: string,
    target: AssistantTarget | null,
  ) => void;
  setModel: (id: string, modelId: string | null) => void;
  setDraft: (id: string, draft: string) => void;
  setLive: (tabId: string, turn: LiveTurn | null) => void;
  updateLive: (tabId: string, update: (turn: LiveTurn) => LiveTurn) => void;
  patchRun: (attemptId: string, patch: Partial<AttemptRun>) => void;
  setTurnContext: (turnId: string, context: TurnContext) => void;
  requestFocus: () => void;
}

const newTabId = () => `conversation|${crypto.randomUUID()}`;

const blankTab = (target: AssistantTarget | null): ConversationTab => ({
  id: newTabId(),
  target,
  conversationId: null,
  modelId: null,
  draft: '',
});

/** A aba ainda não perguntou nada: dá para trocar o alvo sem perder conversa */
const isBlank = (tab: ConversationTab, live: Record<string, LiveTurn>) =>
  tab.conversationId === null && !live[tab.id];

const withoutKey = <T>(record: Record<string, T>, key: string) => {
  if (!(key in record)) return record;
  const next = { ...record };
  delete next[key];
  return next;
};

export const useAssistantStore = create<AssistantState>((set, get) => ({
  panelOpen: false,
  tabs: [],
  activeTabId: null,
  live: {},
  runs: {},
  turnContexts: {},
  focusRequest: 0,

  openPanel: target => {
    const { tabs, activeTabId, live } = get();
    const active = tabs.find(tab => tab.id === activeTabId);

    if (!active) {
      const tab = blankTab(target ?? null);
      set(state => ({
        panelOpen: true,
        tabs: [...state.tabs, tab],
        activeTabId: tab.id,
        focusRequest: state.focusRequest + 1,
      }));
      return;
    }

    if (target && isBlank(active, live)) {
      set(state => ({
        panelOpen: true,
        tabs: state.tabs.map(tab =>
          tab.id === active.id ? { ...tab, target } : tab,
        ),
        focusRequest: state.focusRequest + 1,
      }));
      return;
    }

    if (target) {
      const tab = blankTab(target);
      set(state => ({
        panelOpen: true,
        tabs: [...state.tabs, tab],
        activeTabId: tab.id,
        focusRequest: state.focusRequest + 1,
      }));
      return;
    }

    set(state => ({
      panelOpen: true,
      focusRequest: state.focusRequest + 1,
    }));
  },

  closePanel: () => set({ panelOpen: false }),

  togglePanel: () => {
    if (get().panelOpen) get().closePanel();
    else get().openPanel();
  },

  newTab: target => {
    const { tabs, activeTabId } = get();
    const inherited =
      target === undefined
        ? (tabs.find(tab => tab.id === activeTabId)?.target ?? null)
        : target;
    const tab = blankTab(inherited);
    set(state => ({
      tabs: [...state.tabs, tab],
      activeTabId: tab.id,
      focusRequest: state.focusRequest + 1,
    }));
    return tab.id;
  },

  closeTab: id => {
    set(state => {
      const closedIndex = state.tabs.findIndex(tab => tab.id === id);
      if (closedIndex === -1) return state;

      const tabs = state.tabs.filter(tab => tab.id !== id);
      let activeTabId = state.activeTabId;
      if (activeTabId === id) {
        activeTabId = tabs[Math.min(closedIndex, tabs.length - 1)]?.id ?? null;
      }
      return { tabs, activeTabId, live: withoutKey(state.live, id) };
    });
  },

  setActiveTab: id => {
    if (get().tabs.some(tab => tab.id === id)) set({ activeTabId: id });
  },

  setTarget: (id, target) => {
    set(state => ({
      tabs: state.tabs.map(tab => (tab.id === id ? { ...tab, target } : tab)),
    }));
  },

  setConversation: (id, conversationId) => {
    set(state => ({
      tabs: state.tabs.map(tab =>
        tab.id === id ? { ...tab, conversationId } : tab,
      ),
    }));
  },

  openConversation: (conversationId, target) => {
    const { tabs, activeTabId, live } = get();
    const open = tabs.find(tab => tab.conversationId === conversationId);
    if (open) {
      set({ activeTabId: open.id, panelOpen: true });
      return;
    }

    const active = tabs.find(tab => tab.id === activeTabId);
    if (active && isBlank(active, live)) {
      set(state => ({
        panelOpen: true,
        tabs: state.tabs.map(tab =>
          tab.id === active.id ? { ...tab, conversationId, target } : tab,
        ),
      }));
      return;
    }

    const tab = { ...blankTab(target), conversationId };
    set(state => ({
      panelOpen: true,
      tabs: [...state.tabs, tab],
      activeTabId: tab.id,
    }));
  },

  setModel: (id, modelId) => {
    set(state => ({
      tabs: state.tabs.map(tab => (tab.id === id ? { ...tab, modelId } : tab)),
    }));
  },

  setDraft: (id, draft) => {
    set(state => ({
      tabs: state.tabs.map(tab => (tab.id === id ? { ...tab, draft } : tab)),
    }));
  },

  setLive: (tabId, turn) => {
    set(state => ({
      live: turn
        ? { ...state.live, [tabId]: turn }
        : withoutKey(state.live, tabId),
    }));
  },

  updateLive: (tabId, update) => {
    set(state => {
      const turn = state.live[tabId];
      if (!turn) return state;
      return { live: { ...state.live, [tabId]: update(turn) } };
    });
  },

  patchRun: (attemptId, patch) => {
    set(state => ({
      runs: {
        ...state.runs,
        [attemptId]: { ...(state.runs[attemptId] ?? EMPTY_RUN), ...patch },
      },
    }));
  },

  setTurnContext: (turnId, context) => {
    set(state => ({
      turnContexts: { ...state.turnContexts, [turnId]: context },
    }));
  },

  requestFocus: () => set(state => ({ focusRequest: state.focusRequest + 1 })),
}));

export const useActiveConversationTab = (): ConversationTab | null =>
  useAssistantStore(
    state => state.tabs.find(tab => tab.id === state.activeTabId) ?? null,
  );
