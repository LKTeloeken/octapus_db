import { openUrl } from '@tauri-apps/plugin-opener';
import { useCallback, useRef, useState } from 'react';
import type { Server } from '@/api/types/server.types';
import { useServers } from '@/queries/use-servers';
import { useCommandPaletteStore } from '@/stores/command-palette-store';
import { useImportDialogStore } from '@/stores/import-dialog-store';
import { useServerGroupDialogStore } from '@/stores/server-group-dialog-store';

const REPOSITORY_URL = 'https://github.com/LKTeloeken/octapus_db';

export const useSidebar = () => {
  const [isFormOpen, setIsFormOpen] = useState(false);
  const [editingServer, setEditingServer] = useState<Server | null>(null);
  /** Grupo do servidor novo ("Novo servidor aqui" no menu do grupo) */
  const [createInGroupId, setCreateInGroupId] = useState<number | null>(null);
  const [isSettingsOpen, setSettingsOpen] = useState(false);
  const { data: servers } = useServers();
  const setPaletteOpen = useCommandPaletteStore(state => state.setOpen);
  const openGroupDialog = useServerGroupDialogStore(state => state.open);
  /** Ação escolhida no menu "+", à espera de o menu terminar de fechar */
  const pendingMenuAction = useRef<(() => void) | null>(null);
  const openImport = useImportDialogStore(state => state.open);

  const openCreateForm = useCallback((groupId: number | null) => {
    setEditingServer(null);
    setCreateInGroupId(groupId);
    setIsFormOpen(true);
  }, []);

  const openCreateGroup = useCallback(
    () => openGroupDialog({ kind: 'create' }),
    [openGroupDialog],
  );

  const openEditForm = useCallback((server: Server) => {
    setEditingServer(server);
    setIsFormOpen(true);
  }, []);

  // O servidor em edição fica guardado depois de fechar: o diálogo ainda
  // aparece durante a animação de saída, e limpar aqui trocaria o título e o
  // rodapé para os de "Adicionar" no meio do fade. Os dois `open*` definem o
  // servidor antes de abrir de novo.
  const closeForm = useCallback(() => {
    setIsFormOpen(false);
  }, []);

  /**
   * Os itens do menu "+" abrem diálogos modais, e abrir o diálogo enquanto o
   * menu ainda fecha deixava as duas camadas do Radix brigando: o foco caía no
   * body (Esc parava de funcionar) e o `pointer-events: none` do body podia
   * sobrar depois de fechar o diálogo, travando o app. O item só marca a
   * ação; ela roda quando o menu terminou de fechar (`onCloseAutoFocus`).
   */
  const fromMenu = useCallback(
    (action: () => void) => () => {
      pendingMenuAction.current = action;
    },
    [],
  );

  const onMenuCloseAutoFocus = useCallback((event: Event) => {
    const action = pendingMenuAction.current;
    pendingMenuAction.current = null;
    if (!action) return;
    // O foco vai para o diálogo, não de volta ao "+"
    event.preventDefault();
    action();
  }, []);

  const openPalette = useCallback(() => setPaletteOpen(true), [setPaletteOpen]);

  // Abre no navegador do sistema: um <a target="_blank"> no webview do Tauri
  // não sai do app.
  const openRepository = useCallback(() => {
    void openUrl(REPOSITORY_URL);
  }, []);

  return {
    isFormOpen,
    editingServer,
    createInGroupId,
    serverCount: servers?.length ?? 0,
    isSettingsOpen,
    setSettingsOpen,
    openCreateForm,
    openCreateGroup,
    openImport,
    fromMenu,
    onMenuCloseAutoFocus,
    openEditForm,
    closeForm,
    openPalette,
    openRepository,
  };
};
