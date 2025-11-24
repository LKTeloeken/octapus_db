import type { TreeNode } from "@/shared/models/database.types";
import type { Server, ServerPrimitive } from "@/shared/models/servers.types";

export interface UserServersProps {
  addChildren: (childrens: TreeNode[]) => void;
  removeNode: (nodeId: string) => void;
}

export type AddServer = (serverData: ServerPrimitive) => Promise<void>;

export type EditServer = (server: Server) => Promise<void>;

export type DeleteServer = (serverId: number) => Promise<void>;
