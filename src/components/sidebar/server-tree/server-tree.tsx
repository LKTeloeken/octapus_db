import { memo, useMemo } from "react";
import { TreeNode } from "../tree-node/tree-node";
import type { ServerTreeProps } from "./server-tree.types";

export const ServerTree = memo(
  ({ nodes, childrenMap, toggleNode }: ServerTreeProps) => {
    const rootNodeIds = useMemo(() => {
      return Array.from(nodes.values())
        .filter((node) => !node.parentId)
        .map((node) => node.id);
    }, [nodes]);

    return (
      <div className="p-2">
        <div className="border rounded-lg p-4 ">
          {rootNodeIds.length === 0 ? (
            <p className="text-gray-500">No servers connected</p>
          ) : (
            rootNodeIds.map((rootId) => (
              <TreeNode
                key={rootId}
                nodeId={rootId}
                nodes={nodes}
                childrenMap={childrenMap}
                onToggle={toggleNode}
              />
            ))
          )}
        </div>
      </div>
    );
  }
);
