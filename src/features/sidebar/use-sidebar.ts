import { openUrl } from '@tauri-apps/plugin-opener';
import { useCallback, useState } from 'react';
import type { Server } from '@/api/types/server.types';
import { useServers } from '@/queries/use-servers';
import { useCommandPaletteStore } from '@/stores/command-palette-store';

const REPOSITORY_URL = 'https://github.com/LKTeloeken/octapus_db';

export const useSidebar = () => {
  const [isFormOpen, setIsFormOpen] = useState(false);
  const [editingServer, setEditingServer] = useState<Server | null>(null);
  const [isSettingsOpen, setSettingsOpen] = useState(false);
  const { data: servers } = useServers();
  const setPaletteOpen = useCommandPaletteStore(state => state.setOpen);

  const openCreateForm = useCallback(() => {
    setEditingServer(null);
    setIsFormOpen(true);
  }, []);

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

  const openPalette = useCallback(() => setPaletteOpen(true), [setPaletteOpen]);

  // Abre no navegador do sistema: um <a target="_blank"> no webview do Tauri
  // não sai do app.
  const openRepository = useCallback(() => {
    void openUrl(REPOSITORY_URL);
  }, []);

  return {
    isFormOpen,
    editingServer,
    serverCount: servers?.length ?? 0,
    isSettingsOpen,
    setSettingsOpen,
    openCreateForm,
    openEditForm,
    closeForm,
    openPalette,
    openRepository,
  };
};
