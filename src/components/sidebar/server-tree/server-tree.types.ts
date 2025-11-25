import type { OpenTab } from "@/shared/hooks/use-query-tabs/use-query-tabs.types";
import type { TreeNode } from "@/shared/models/database.types";
import type { Server } from "@/shared/models/servers.types";

export interface ServerTreeProps {
  nodes: Map<string, TreeNode>;
  childrenMap: Map<string, string[]>;
  toggleNode: (nodeId: string) => void;
  openServerModal: (server: Server) => void;
  openNewTab: OpenTab;
}
