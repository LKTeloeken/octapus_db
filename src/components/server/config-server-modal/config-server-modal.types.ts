import type { Server, ServerPrimitive } from "@/shared/models/servers.types";

export type ConfigServerModalProps =
  | {
      isEditMode: false;
      serverData: null;
      isOpen: boolean;
      isLoading: boolean;
      onClose: () => void;
      onCreateServer: (server: ServerPrimitive) => Promise<void>;
      onRemoveServer: (serverId: number) => Promise<void>;
    }
  | {
      isEditMode: true;
      serverData: ServerPrimitive;
      serverId: number;
      isOpen: boolean;
      isLoading: boolean;
      onClose: () => void;
      onEditServer: (server: Server) => Promise<void>;
      onRemoveServer: (serverId: number) => Promise<void>;
    };
