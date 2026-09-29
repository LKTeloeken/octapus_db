import { create } from 'zustand';

/**
 * Aberto/fechado da paleta de comandos. Vive num store (e não no hook da
 * paleta) porque outros pontos do shell também a abrem: a busca da sidebar e
 * o botão "Comandos" da barra de abas, além do Cmd/Ctrl+K.
 */
interface CommandPaletteState {
  isOpen: boolean;
  setOpen: (open: boolean) => void;
  toggle: () => void;
}

export const useCommandPaletteStore = create<CommandPaletteState>(set => ({
  isOpen: false,
  setOpen: open => set({ isOpen: open }),
  toggle: () => set(state => ({ isOpen: !state.isOpen })),
}));
