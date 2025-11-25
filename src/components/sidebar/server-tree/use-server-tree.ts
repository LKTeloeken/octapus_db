import type { TreeNode } from "@/shared/models/database.types";
import { useMemo } from "react";

export const useServerTree = (nodes: Map<string, TreeNode>) => {
  const rootNodeIds = useMemo(() => {
    return Array.from(nodes.values())
      .filter((node) => !node.parentId)
      .map((node) => node.id);
  }, [nodes]);

  return { rootNodeIds };
};
