import type { TreeNodeType, TreeNode } from "@/shared/models/database.types";

export interface UseDataStructureProps {
  getChildren: (nodeType: TreeNodeType, nodeId: string) => Promise<TreeNode[]>;
}

export interface TreeState {
  nodes: Map<string, TreeNode>;
  childrenMap: Map<string, string[]>; // parentId -> childIds
}
