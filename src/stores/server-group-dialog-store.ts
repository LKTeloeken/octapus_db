import { create } from 'zustand';
import type { Server, ServerGroup } from '@/api/types/server.types';

/** Os diálogos de grupo de servidores (menu "+" da sidebar e menu da árvore) */
export type ServerGroupDialog =
  /** Grupo novo; com `moveServerId`, o servidor vai para ele ao criar */
  | { kind: 'create'; moveServerId?: number }
  | { kind: 'rename'; group: ServerGroup }
  | { kind: 'delete'; group: ServerGroup; serverCount: number }
  /** Mover um servidor para outro grupo (ou para fora de grupo) */
  | { kind: 'move'; server: Server };

interface ServerGroupDialogState {
  dialog: ServerGroupDialog | null;
  isOpen: boolean;
  /** Conta as aberturas: reabrir começa um formulário novo */
  openCount: number;
  open: (dialog: ServerGroupDialog) => void;
  close: () => void;
}

/**
 * Igual ao `backup-dialog-store`: o diálogo fica guardado depois de fechar,
 * para a animação de saída não trocar o conteúdo no meio.
 */
export const useServerGroupDialogStore = create<ServerGroupDialogState>(
  set => ({
    dialog: null,
    isOpen: false,
    openCount: 0,
    open: dialog =>
      set(state => ({ dialog, isOpen: true, openCount: state.openCount + 1 })),
    close: () => set({ isOpen: false }),
  }),
);
