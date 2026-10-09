import { create } from 'zustand';

/**
 * Aberto/fechado da paleta de comandos. Vive num store (e não no hook da
 * paleta) porque outros pontos do shell também a abrem: a busca da sidebar e
 * o botão "Comandos" da barra de abas, além do Cmd/Ctrl+K.
 */
interface CommandPaletteState {
  isOpen: boolean;
  /**
   * Schema fixado com Tab: a busca fica só nas relações dele. Sobrevive ao
   * fechar a paleta — sai com Backspace no campo vazio ou Shift+Tab.
   */
  pinnedSchema: string | null;
  setOpen: (open: boolean) => void;
  toggle: () => void;
  setPinnedSchema: (schema: string | null) => void;
}

export const useCommandPaletteStore = create<CommandPaletteState>(set => ({
  isOpen: false,
  pinnedSchema: null,
  setOpen: open => set({ isOpen: open }),
  toggle: () => set(state => ({ isOpen: !state.isOpen })),
  setPinnedSchema: pinnedSchema => set({ pinnedSchema }),
}));
