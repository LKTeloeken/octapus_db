import type { TreeNodeType } from "@/shared/models/database.types";

export interface UseDataStructureProps {
  getChildren: (nodeType: TreeNodeType, nodeId: string) => Promise<TreeNode[]>;
}

export interface TreeNode {
  id: string;
  type: TreeNodeType;
  name: string;
  parentId: string | null;
  hasChildren: boolean;
  isExpanded: boolean;
  isLoading: boolean;
  metadata?: Record<string, any>; // type info, constraints, etc.
}

export interface TreeState {
  nodes: Map<string, TreeNode>;
  childrenMap: Map<string, string[]>; // parentId -> childIds
}
