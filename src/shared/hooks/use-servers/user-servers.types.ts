import type { TreeNode } from "@/shared/models/database.types";
import type { Server, ServerPrimitive } from "@/shared/models/servers.types";
import type {
  AddNodes,
  RemoveNode,
} from "../use-data-structure/use-data-structure.types";

export interface UserServersProps {
  addChildren: AddNodes;
  removeNode: RemoveNode;
}

export type AddServer = (serverData: ServerPrimitive) => Promise<void>;

export type EditServer = (server: Server) => Promise<void>;

export type DeleteServer = (serverId: number) => Promise<void>;
