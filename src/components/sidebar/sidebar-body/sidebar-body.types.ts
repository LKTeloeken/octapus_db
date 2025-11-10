import type { TreeNode } from "@/shared/models/database.types";

export interface SidebarBodyProps {
  nodes: Map<string, TreeNode>;
  childrenMap: Map<string, string[]>;
  isLoading: boolean;
  toggleNode: (nodeId: string) => void;
  onCreateServer: () => void;
  onEditServer: (serverId: string) => void;
  onDeleteServer: (serverId: string) => void;
}
