import type { TreeNode } from "@/shared/models/database.types";

export interface ServerTreeProps {
  nodes: Map<string, TreeNode>;
  childrenMap: Map<string, string[]>;
  toggleNode: (nodeId: string) => void;
}
