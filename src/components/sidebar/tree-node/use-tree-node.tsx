import { HugeiconsIcon } from "@hugeicons/react";
import { ComputerIcon, DatabaseIcon, Folder01Icon, HashtagIcon, TableIcon } from "@hugeicons/core-free-icons";
import type { TreeNode, TreeNodeType } from '@/shared/models/database.types';
import type { Server } from '@/shared/models/servers.types';
import type { OpenTab } from '@/shared/hooks/use-query-tabs/use-query-tabs.types';
import { TabType } from '@/shared/models/tabs.types';
import { cn } from '@/lib/utils';

export const useTreeNode = (
  node: TreeNode,
  nodeId: string,
  nodes: Map<string, TreeNode>,
  childrenMap: Map<string, string[]>,
  openServerModal: (server: Server) => void,
  openNewTab: OpenTab,
) => {
  const childrenIds = childrenMap.get(nodeId) || [];
  const hasChildren = node.hasChildren;
  const isExpanded = node.isExpanded;
  const metadata = node.metadata;

  const handleServerEdit = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (
      openServerModal &&
      metadata &&
      metadata.type === 'server' &&
      metadata.serverData
    ) {
      openServerModal(metadata.serverData);
    }
  };

  const handleOpenNewTab = (e: React.MouseEvent) => {
    e.stopPropagation();

    const { type, serverId } = metadata;
    const quoteIdentifier = (value: string) => `"${value.replace(/"/g, '""')}"`;
    const resolveSeedQuery = () => {
      if (type !== 'table' || !metadata.schemaName || !metadata.tableName) return '';
      return `SELECT * FROM ${quoteIdentifier(metadata.schemaName)}.${quoteIdentifier(metadata.tableName)}`;
    };

    if (type === 'server' && serverId) {
      const loadedDatabaseName = childrenIds
        .map(childId => nodes.get(childId))
        .find(child => child?.type === 'database')?.name;

      // Fallback order for server SQL tabs: metadata default DB > first loaded DB > postgres.
      openNewTab(
        serverId,
        metadata.serverData?.default_database || loadedDatabaseName || 'postgres',
        {
          type: TabType.Query,
          content: '',
        },
      );
    }

    if (type !== 'server' && serverId) {
      openNewTab(serverId, metadata.databaseName, {
        type: TabType.Query,
        content: resolveSeedQuery(),
      });
    }
  };

  const getNodeIcon = (type: TreeNodeType, isConnected: boolean) => {
    switch (type) {
      case 'server':
        return (
          <HugeiconsIcon
            icon={ComputerIcon}
            className={cn('size-4', isConnected && 'text-primary')}
          />
        );
      case 'database':
        return <HugeiconsIcon icon={DatabaseIcon} className="size-4" />;
      case 'schema':
        return <HugeiconsIcon icon={Folder01Icon} className="size-4" />;
      case 'table':
        return <HugeiconsIcon icon={TableIcon} className="size-4" />;
      case 'column':
        return <HugeiconsIcon icon={HashtagIcon} className="size-4" />;
      default:
        return null;
    }
  };

  return {
    childrenIds,
    hasChildren,
    isExpanded,
    metadata,
    handleServerEdit,
    getNodeIcon,
    handleOpenNewTab,
  };
};
