import { create } from 'zustand';

interface VaultStore {
  /** Diálogo de destravar, aberto quando o backend reporta o cofre trancado */
  isUnlockOpen: boolean;
  openUnlock: () => void;
  closeUnlock: () => void;
}

export const useVaultStore = create<VaultStore>(set => ({
  isUnlockOpen: false,
  openUnlock: () => set({ isUnlockOpen: true }),
  closeUnlock: () => set({ isUnlockOpen: false }),
}));
