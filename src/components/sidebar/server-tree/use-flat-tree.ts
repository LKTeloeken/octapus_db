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
  searchTerm: string,
): FlatTreeItem[] => {
  return useMemo(() => {
    const result: FlatTreeItem[] = [];
    const normalizedSearch = searchTerm.trim().toLowerCase();
    const descendantMatchCache = new Map<string, boolean>();

    const rootNodeIds = Array.from(nodes.values())
      .filter(node => !node.parentId)
      .map(node => node.id);

    const nodeMatches = (nodeId: string) => {
      if (!normalizedSearch) return true;
      const node = nodes.get(nodeId);
      if (!node) return false;
      const target = `${node.type} ${node.name}`.toLowerCase();
      return target.includes(normalizedSearch);
    };

    const hasMatchingDescendant = (nodeId: string): boolean => {
      const cached = descendantMatchCache.get(nodeId);
      if (cached !== undefined) return cached;

      const children = childrenMap.get(nodeId) || [];
      const hasMatch = children.some(childId => {
        if (nodeMatches(childId)) return true;
        return hasMatchingDescendant(childId);
      });
      descendantMatchCache.set(nodeId, hasMatch);
      return hasMatch;
    };

    const walk = (nodeId: string, level: number, isLastChild: boolean) => {
      const node = nodes.get(nodeId);
      if (!node) return;
      const match = nodeMatches(nodeId);
      const childMatch = hasMatchingDescendant(nodeId);

      if (normalizedSearch && !match && !childMatch) {
        return;
      }

      result.push({ nodeId, level, isLastChild });

      if (normalizedSearch || node.isExpanded) {
        const children = (childrenMap.get(nodeId) || []).filter(
          childId =>
            !normalizedSearch ||
            nodeMatches(childId) ||
            hasMatchingDescendant(childId),
        );
        children.forEach((childId, index) => {
          walk(childId, level + 1, index === children.length - 1);
        });
      }
    };

    rootNodeIds.forEach((id, index) => {
      walk(id, 0, index === rootNodeIds.length - 1);
    });

    return result;
  }, [childrenMap, nodes, searchTerm]);
};
