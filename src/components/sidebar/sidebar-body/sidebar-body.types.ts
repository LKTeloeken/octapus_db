import type { TreeNode } from "@/shared/models/database.types";
import type {
  AddServer,
  EditServer,
  DeleteServer,
} from "@/shared/hooks/use-servers/user-servers.types";

export interface SidebarBodyProps {
  nodes: Map<string, TreeNode>;
  childrenMap: Map<string, string[]>;
  isLoading: boolean;
  toggleNode: (nodeId: string) => void;
  onCreateServer: AddServer;
  onEditServer: EditServer;
  onDeleteServer: DeleteServer;
}
