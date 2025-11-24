import { useState } from "react";
import type { Server } from "@/shared/models/servers.types";

export const useSidebarBody = () => {
  const [isConfigServerModalOpen, setIsConfigServerModalOpen] =
    useState<boolean>(false);
  const [editingServer, setEditingServer] = useState<Server | null>(null);

  const openConfigServerModal = (server?: Server) => {
    if (server) {
      setEditingServer(server);
    } else {
      setEditingServer(null);
    }
    setIsConfigServerModalOpen(true);
  };

  const closeConfigServerModal = () => {
    setIsConfigServerModalOpen(false);
    setEditingServer(null);
  };

  return {
    isConfigServerModalOpen,
    editingServer,
    openConfigServerModal,
    closeConfigServerModal,
  };
};
