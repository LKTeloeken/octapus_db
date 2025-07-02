import { useMemo } from "react";
import { ITreeNode } from "@/shared/models/tree";

export interface NestedNode {
  name: string;
  type: string;
  itemKey: string;
  data?: unknown;
  children?: NestedNode[];
}

const useTree = (nodesMap: Record<string, ITreeNode>): NestedNode[] => {
  return useMemo(() => {
    const cache = new Map<string, NestedNode>();

    // Build one node (and its subtree) by key
    function build(key: string): NestedNode {
      if (cache.has(key)) {
        return cache.get(key)!;
      }
      const { name, data, children } = nodesMap[key];
      const [type] = key.split("::");
      const node: NestedNode = {
        name,
        data,
        type,
        itemKey: key,
      };
      cache.set(key, node);

      if (children && children.length > 0) {
        node.children = children.map((childKey) => build(childKey));
      }
      return node;
    }

    // Find all keys that are *not* referenced as someone's child → roots
    const allKeys = Object.keys(nodesMap);
    const childKeys = new Set<string>();
    allKeys.forEach((k) =>
      nodesMap[k].children?.forEach((c) => childKeys.add(c))
    );
    const rootKeys = allKeys.filter((k) => !childKeys.has(k));

    // Build each root subtree
    return rootKeys.map((rootKey) => build(rootKey));
  }, [nodesMap]);
};

export default useTree;
