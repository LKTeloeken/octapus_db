import type { TreeNode } from "@/shared/models/database.types";

export interface TreeNodeProps {
  nodeId: string;
  nodes: Map<string, TreeNode>;
  childrenMap: Map<string, string[]>;
  onToggle: (nodeId: string) => void;
  level?: number;
  isLastChild?: boolean;
}
