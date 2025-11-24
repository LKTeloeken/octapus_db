import type { TreeNode } from "@/shared/models/database.types";
import type { Server } from "@/shared/models/servers.types";

export interface ServerTreeProps {
  nodes: Map<string, TreeNode>;
  childrenMap: Map<string, string[]>;
  toggleNode: (nodeId: string) => void;
  onNodeClick: (server: Server) => void;
}
