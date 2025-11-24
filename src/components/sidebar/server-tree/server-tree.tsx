import { memo, useMemo } from "react";
import { TreeNode } from "../tree-node/tree-node";
import type { ServerTreeProps } from "./server-tree.types";

export const ServerTree = memo(
  ({ nodes, childrenMap, toggleNode, onNodeClick }: ServerTreeProps) => {
    const rootNodeIds = useMemo(() => {
      return Array.from(nodes.values())
        .filter((node) => !node.parentId)
        .map((node) => node.id);
    }, [nodes]);

    return (
      <div className="p-2 h-full overflow-auto">
        {rootNodeIds.length === 0 ? (
          <p className="text-sm text-muted-foreground">No servers connected</p>
        ) : (
          <div className="space-y-1">
            {rootNodeIds.map((rootId, index) => (
              <TreeNode
                key={rootId}
                nodeId={rootId}
                nodes={nodes}
                childrenMap={childrenMap}
                onToggle={toggleNode}
                level={0}
                isLastChild={index === rootNodeIds.length - 1}
                onNodeClick={onNodeClick}
              />
            ))}
          </div>
        )}
      </div>
    );
  }
);
