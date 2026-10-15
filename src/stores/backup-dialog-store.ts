import { create } from 'zustand';
import type { BackupTarget } from '@/api/types/backup.types';

/** Os diálogos de backup/restauração abertos pelo menu da árvore */
export type BackupDialog =
  | {
      kind: 'backup';
      serverId: number;
      database: string;
      target: BackupTarget;
    }
  /** Restaurar um arquivo neste database */
  | { kind: 'restore'; serverId: number; database: string };

interface BackupDialogState {
  dialog: BackupDialog | null;
  isOpen: boolean;
  /** Conta as aberturas: reabrir começa um formulário novo */
  openCount: number;
  open: (dialog: BackupDialog) => void;
  close: () => void;
}

/**
 * Igual ao `structure-editor-store`: o diálogo fica guardado depois de fechar,
 * para a animação de saída não trocar o conteúdo no meio.
 */
export const useBackupDialogStore = create<BackupDialogState>(set => ({
  dialog: null,
  isOpen: false,
  openCount: 0,
  open: dialog =>
    set(state => ({ dialog, isOpen: true, openCount: state.openCount + 1 })),
  close: () => set({ isOpen: false }),
}));
