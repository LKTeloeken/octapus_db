import { create } from 'zustand';

/**
 * Célula de uma linha copiada: `undefined` é "sem valor" (linha nova em que a
 * coluna não foi tocada) — ao colar, o banco aplica o default.
 */
export type CopiedCell = string | null | undefined;

/** Linhas inteiras copiadas de uma grade (Cmd/Ctrl+C com linhas marcadas) */
export interface CopiedRows {
  /** O texto que foi para a área de transferência: colar só vale se ainda for ele */
  text: string;
  /** Aba de origem: colar na mesma grade é duplicar (defaults ficam vazios) */
  scopeId: string;
  /** Todas as colunas da origem (inclusive as ocultas), por nome */
  columns: string[];
  rows: CopiedCell[][];
}

/**
 * As linhas copiadas da grade, com a estrutura que o texto perde (colunas,
 * NULL × vazio, default). Fica fora das abas: copiar numa e colar noutra é o
 * caso de uso. Some quando outra coisa vai para a área de transferência.
 */
interface GridClipboardState {
  copied: CopiedRows | null;
  setCopied: (copied: CopiedRows | null) => void;
}

export const useGridClipboardStore = create<GridClipboardState>(set => ({
  copied: null,
  setCopied: copied => set({ copied }),
}));
