import { create } from 'zustand';

/**
 * Sidebar tree UI state — quais nós estão expandidos e qual é o nó sob o cursor
 * do teclado. A tree DATA vem da camada queries/ por nível.
 */
interface TreeState {
  expanded: Set<string>;
  /** Nó sob o cursor do teclado (navegação por setas), ou null */
  focusedNodeId: string | null;
  toggleNode: (nodeId: string) => void;
  expandNode: (nodeId: string) => void;
  collapseNode: (nodeId: string) => void;
  /** Colapsa vários nós de uma vez (refresh de estrutura fecha as tabelas do escopo) */
  collapseNodes: (nodeIds: string[]) => void;
  setFocusedNode: (nodeId: string | null) => void;
  /**
   * Filtro digitado num nó com muitos filhos (5.000 schemas de tenant). Mora
   * aqui e não no input: a linha do filtro sai do DOM quando rola para fora da
   * árvore virtualizada.
   */
  filters: Map<string, string>;
  /** Quantos filhos um nó mostra — "carregar mais" aumenta */
  limits: Map<string, number>;
  setFilter: (nodeId: string, value: string) => void;
  showMore: (nodeId: string, current: number, step: number) => void;
  /** Databases cujos schemas aparecem agrupados por formato (multi-tenant) */
  shapeGrouped: Set<string>;
  toggleShapeGrouping: (dbNodeId: string) => void;
}

export const useTreeStore = create<TreeState>(set => ({
  expanded: new Set(),
  focusedNodeId: null,

  toggleNode: nodeId => {
    set(state => {
      const expanded = new Set(state.expanded);
      if (expanded.has(nodeId)) expanded.delete(nodeId);
      else expanded.add(nodeId);
      return { expanded };
    });
  },

  expandNode: nodeId => {
    set(state => {
      if (state.expanded.has(nodeId)) return state;
      const expanded = new Set(state.expanded);
      expanded.add(nodeId);
      return { expanded };
    });
  },

  collapseNode: nodeId => {
    set(state => {
      if (!state.expanded.has(nodeId)) return state;
      const expanded = new Set(state.expanded);
      expanded.delete(nodeId);
      return { expanded };
    });
  },

  collapseNodes: nodeIds => {
    set(state => {
      if (!nodeIds.some(id => state.expanded.has(id))) return state;
      const expanded = new Set(state.expanded);
      for (const nodeId of nodeIds) expanded.delete(nodeId);
      return { expanded };
    });
  },

  setFocusedNode: nodeId => set({ focusedNodeId: nodeId }),

  filters: new Map(),
  limits: new Map(),
  shapeGrouped: new Set(),

  toggleShapeGrouping: dbNodeId => {
    set(state => {
      const shapeGrouped = new Set(state.shapeGrouped);
      if (shapeGrouped.has(dbNodeId)) shapeGrouped.delete(dbNodeId);
      else shapeGrouped.add(dbNodeId);
      return { shapeGrouped };
    });
  },

  setFilter: (nodeId, value) => {
    set(state => {
      const filters = new Map(state.filters);
      if (value) filters.set(nodeId, value);
      else filters.delete(nodeId);
      // Filtro novo começa da primeira janela
      const limits = new Map(state.limits);
      limits.delete(nodeId);
      return { filters, limits };
    });
  },

  showMore: (nodeId, current, step) => {
    set(state => {
      const limits = new Map(state.limits);
      limits.set(nodeId, current + step);
      return { limits };
    });
  },
}));

export const useIsNodeExpanded = (nodeId: string): boolean =>
  useTreeStore(state => state.expanded.has(nodeId));
