import { create } from 'zustand';
import {
  DEFAULT_GRID_SEARCH_OPTIONS,
  type GridSearchOptions,
} from '@/lib/grid-search';

/** Resultado atual da navegação, por posição (sobrevive a uma nova varredura) */
export interface GridSearchCurrent {
  rowIndex: number;
  columnName: string;
}

export interface GridSearchState {
  open: boolean;
  /** Texto digitado, cru (o debounce é do hook) */
  query: string;
  options: GridSearchOptions;
  current: GridSearchCurrent | null;
}

const INITIAL: GridSearchState = {
  open: false,
  query: '',
  options: DEFAULT_GRID_SEARCH_OPTIONS,
  current: null,
};

/**
 * Busca na grade (Cmd/Ctrl+F), por aba: cada aba guarda o próprio painel,
 * texto e opções — trocar de aba e voltar restaura como estava, e nada vaza
 * de uma para outra. Os resultados não moram aqui: são derivados das linhas
 * carregadas e recalculados pelo hook. Limpo quando a aba fecha.
 */
interface GridSearchStoreState {
  byScope: Map<string, GridSearchState>;

  toggle: (scopeId: string) => void;
  setOpen: (scopeId: string, open: boolean) => void;
  setQuery: (scopeId: string, query: string) => void;
  setOptions: (scopeId: string, patch: Partial<GridSearchOptions>) => void;
  setCurrent: (scopeId: string, current: GridSearchCurrent | null) => void;
  clear: (scopeId: string) => void;
}

export const getGridSearch = (
  byScope: Map<string, GridSearchState>,
  scopeId: string,
): GridSearchState => byScope.get(scopeId) ?? INITIAL;

const patchScope = (
  byScope: Map<string, GridSearchState>,
  scopeId: string,
  patch: (state: GridSearchState) => Partial<GridSearchState>,
) => {
  const current = getGridSearch(byScope, scopeId);
  return new Map(byScope).set(scopeId, { ...current, ...patch(current) });
};

export const useGridSearchStore = create<GridSearchStoreState>(set => ({
  byScope: new Map(),

  toggle: scopeId =>
    set(state => ({
      byScope: patchScope(state.byScope, scopeId, s => ({ open: !s.open })),
    })),

  setOpen: (scopeId, open) =>
    set(state => {
      if (getGridSearch(state.byScope, scopeId).open === open) return state;
      return { byScope: patchScope(state.byScope, scopeId, () => ({ open })) };
    }),

  setQuery: (scopeId, query) =>
    set(state => ({
      byScope: patchScope(state.byScope, scopeId, () => ({ query })),
    })),

  setOptions: (scopeId, patch) =>
    set(state => ({
      byScope: patchScope(state.byScope, scopeId, s => ({
        options: { ...s.options, ...patch },
      })),
    })),

  setCurrent: (scopeId, current) =>
    set(state => ({
      byScope: patchScope(state.byScope, scopeId, () => ({ current })),
    })),

  clear: scopeId =>
    set(state => {
      if (!state.byScope.has(scopeId)) return state;
      const byScope = new Map(state.byScope);
      byScope.delete(scopeId);
      return { byScope };
    }),
}));
