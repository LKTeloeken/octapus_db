import { create } from 'zustand';

interface ImportDialogState {
  isOpen: boolean;
  /** Conta as aberturas: reabrir começa do primeiro passo */
  openCount: number;
  open: () => void;
  close: () => void;
}

/** O diálogo de importar conexões (menu "+" da sidebar e árvore vazia) */
export const useImportDialogStore = create<ImportDialogState>(set => ({
  isOpen: false,
  openCount: 0,
  open: () => set(state => ({ isOpen: true, openCount: state.openCount + 1 })),
  close: () => set({ isOpen: false }),
}));
