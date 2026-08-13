import { useCallback, useState } from 'react';
import type { Server } from '@/api/types/server.types';

export const useSidebar = () => {
  const [isFormOpen, setIsFormOpen] = useState(false);
  const [editingServer, setEditingServer] = useState<Server | null>(null);

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

  return {
    isFormOpen,
    editingServer,
    openCreateForm,
    openEditForm,
    closeForm,
  };
};
