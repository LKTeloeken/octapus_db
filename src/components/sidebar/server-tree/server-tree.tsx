// src/components/sidebar/server-tree/server-tree.tsx
import { memo } from 'react';
import { TreeNode } from '../tree-node/tree-node';
import { useFlatTree } from './use-flat-tree';
import { useVirtualization } from '@/shared/hooks/use-virtualization/use-virtualization';
import type { ServerTreeProps } from './server-tree.types';

const ESTIMATED_ROW_HEIGHT = 32;

export const ServerTree = memo(
  ({
    nodes,
    childrenMap,
    toggleNode,
    openServerModal,
    openNewTab,
    searchTerm = '',
  }: ServerTreeProps) => {
    const flatItems = useFlatTree(nodes, childrenMap, searchTerm);

    const { parentRef, rowVirtualizer } = useVirtualization(
      flatItems.length,
      ESTIMATED_ROW_HEIGHT,
    );

    if (flatItems.length === 0) {
      return (
        <p className="text-sm text-muted-foreground">No servers connected</p>
      );
    }

    return (
      <div
        ref={parentRef as React.RefObject<HTMLDivElement>}
        className="h-full overflow-auto"
      >
        <div
          className="relative w-full"
          style={{ height: `${rowVirtualizer.getTotalSize()}px` }}
        >
          {rowVirtualizer.getVirtualItems().map(virtualRow => {
            const item = flatItems[virtualRow.index];
            return (
              <div
                key={item.nodeId}
                className="absolute left-0 top-0 w-full"
                style={{
                  height: `${virtualRow.size}px`,
                  transform: `translateY(${virtualRow.start}px)`,
                }}
              >
                <TreeNode
                  nodeId={item.nodeId}
                  nodes={nodes}
                  childrenMap={childrenMap}
                  onToggle={toggleNode}
                  level={item.level}
                  isLastChild={item.isLastChild}
                  openServerModal={openServerModal}
                  openNewTab={openNewTab}
                />
              </div>
            );
          })}
        </div>
      </div>
    );
  },
);
