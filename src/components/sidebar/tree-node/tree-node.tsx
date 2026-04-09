import { HugeiconsIcon } from "@hugeicons/react";
import { Add01Icon, ArrowDown01Icon, ArrowRight01Icon, Edit01Icon } from "@hugeicons/core-free-icons";
import { memo } from 'react';
import { Spinner } from '@/components/ui/spinner';
import { useTreeNode } from './use-tree-node';
import type { TreeNodeProps } from './tree-node.types';
import { useStyles } from './tree-node.styles';
import { cn } from '@/lib/utils';
import { Button } from '@/components/ui/button';

export const TreeNode = memo(
  ({
    nodeId,
    nodes,
    childrenMap,
    onToggle,
    level = 0,
    openServerModal,
    openNewTab,
  }: TreeNodeProps) => {
    const node = nodes.get(nodeId);

    if (!node) return null;

    const {
      hasChildren,
      isExpanded,
      metadata,
      getNodeIcon,
      handleServerEdit,
      handleOpenNewTab,
    } = useTreeNode(
      node,
      nodeId,
      nodes,
      childrenMap,
      openServerModal,
      openNewTab,
    );

    const styles = useStyles();

    return (
      <div
        className={styles.container}
        style={{ paddingLeft: `${level * 1.25 + 0.5}rem` }}
        onClick={() => hasChildren && onToggle(nodeId)}
      >
        <div className={styles.iconWrapper}>
          {hasChildren && (
            <>
              {node.isLoading ? (
                <Spinner className={styles.spinner} />
              ) : isExpanded ? (
                <HugeiconsIcon icon={ArrowDown01Icon} className={styles.chevronIcon} />
              ) : (
                <HugeiconsIcon icon={ArrowRight01Icon} className={styles.chevronIcon} />
              )}
            </>
          )}
        </div>

        <div className={styles.contentWrapper}>
          <div className={styles.nodeIconWrapper}>
            {getNodeIcon(node.type, !!node.isConnected)}
          </div>

          <div className={styles.textWrap}>
            <div className={styles.nameText}>{node.name}</div>
            {metadata && metadata.type === 'column' && (
              <div className={styles.dataTypeText}>{metadata.dataType}</div>
            )}
          </div>
        </div>

        {node.type === 'server' ? (
          <div className={cn(styles.menuButtonWrapper, 'opacity-0 group-hover:opacity-100')}>
            <Button
              variant="ghost"
              size="icon"
              className={styles.menuButton}
              onClick={e => {
                handleServerEdit(e);
              }}
            >
              <HugeiconsIcon icon={Edit01Icon} className="h-4 w-4" />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              className={styles.menuButton}
              onClick={e => {
                handleOpenNewTab(e);
              }}
            >
              <HugeiconsIcon icon={Add01Icon} className="h-4 w-4" />
            </Button>
          </div>
        ) : null}
      </div>
    );
  },
);

TreeNode.displayName = 'TreeNode';
