import type { TreeNode } from "@/shared/models/database.types";

export interface TreeState {
  nodes: Map<string, TreeNode>;
  childrenMap: Map<string, string[]>; // parentId -> childIds
}

export type AddNodes = (childrens: TreeNode[]) => void;
export type RemoveNode = (nodeId: string) => void;
export type HandleFetchStructure = (
  serverId: number,
  databaseName: string
) => Promise<void>;
