import type {
  TreeNode,
  TreeNodeType,
  TreeNodeMetadata,
} from "@/shared/models/database.types";

export function formatTreeNode(
  id: string,
  type: TreeNodeType,
  serverId: number,
  name: string,
  parentId: string | null,
  metaData: Partial<TreeNodeMetadata> = {}
): TreeNode {
  const hasChildren = type !== "column";

  const metadata = {
    type,
    serverId,
    ...metaData,
  } as TreeNodeMetadata;

  return {
    id,
    type,
    name,
    parentId,
    hasChildren,
    isExpanded: false,
    isLoading: false,
    metadata,
  };
}
