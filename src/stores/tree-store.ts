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
}));

export const useIsNodeExpanded = (nodeId: string): boolean =>
  useTreeStore(state => state.expanded.has(nodeId));
