import { useCallback, useState } from 'react';
import type { Server } from '@/api/types/server.types';

export const useSidebar = () => {
  const [isFormOpen, setIsFormOpen] = useState(false);
  const [editingServer, setEditingServer] = useState<Server | null>(null);
  const [isSettingsOpen, setIsSettingsOpen] = useState(false);

  const openCreateForm = useCallback(() => {
    setEditingServer(null);
    setIsFormOpen(true);
  }, []);

  const openEditForm = useCallback((server: Server) => {
    setEditingServer(server);
    setIsFormOpen(true);
  }, []);

  const closeForm = useCallback(() => {
    setIsFormOpen(false);
    setEditingServer(null);
  }, []);

  const openSettings = useCallback(() => setIsSettingsOpen(true), []);
  const closeSettings = useCallback(() => setIsSettingsOpen(false), []);

  return {
    isFormOpen,
    editingServer,
    isSettingsOpen,
    openCreateForm,
    openEditForm,
    closeForm,
    openSettings,
    closeSettings,
  };
};
