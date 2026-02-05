import { memo } from 'react';
import { TreeNode } from '../tree-node/tree-node';
import { useServerTree } from './use-server-tree';
import type { ServerTreeProps } from './server-tree.types';

export const ServerTree = memo(
  ({
    nodes,
    childrenMap,
    toggleNode,
    openServerModal,
    openNewTab,
  }: ServerTreeProps) => {
    const { rootNodeIds } = useServerTree(nodes);

    return (
      <div className="h-full overflow-auto">
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
                openServerModal={openServerModal}
                openNewTab={openNewTab}
              />
            ))}
          </div>
        )}
      </div>
    );
  },
);
