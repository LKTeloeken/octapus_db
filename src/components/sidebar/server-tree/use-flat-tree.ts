import { useMemo } from 'react';
import type { TreeNode } from '@/shared/models/database.types';

export interface FlatTreeItem {
  nodeId: string;
  level: number;
  isLastChild: boolean;
}

export const useFlatTree = (
  nodes: Map<string, TreeNode>,
  childrenMap: Map<string, string[]>,
): FlatTreeItem[] => {
  return useMemo(() => {
    const result: FlatTreeItem[] = [];

    const rootNodeIds = Array.from(nodes.values())
      .filter(node => !node.parentId)
      .map(node => node.id);

    const walk = (nodeId: string, level: number, isLastChild: boolean) => {
      const node = nodes.get(nodeId);
      if (!node) return;

      result.push({ nodeId, level, isLastChild });

      if (node.isExpanded) {
        const children = childrenMap.get(nodeId) || [];
        children.forEach((childId, index) => {
          walk(childId, level + 1, index === children.length - 1);
        });
      }
    };

    rootNodeIds.forEach((id, index) => {
      walk(id, 0, index === rootNodeIds.length - 1);
    });

    return result;
  }, [nodes, childrenMap]);
};
