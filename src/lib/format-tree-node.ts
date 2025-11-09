import type { TreeNodeType } from "@/shared/models/database.types";

export function formatTreeNode(
  id: string,
  type: TreeNodeType,
  serverId: number,
  name: string,
  parentId: string | null,
  metaData?: Record<string, any>
) {
  const hasChildren = type !== "column";

  return {
    id,
    type,
    name,
    parentId,
    hasChildren,
    isExpanded: false,
    isLoading: false,
    metadata: { serverId, ...(metaData || {}) },
  };
}
