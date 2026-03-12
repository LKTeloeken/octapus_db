import { HugeiconsIcon } from "@hugeicons/react";
import { DatabaseIcon, Folder01Icon, HashtagIcon, TableIcon } from "@hugeicons/core-free-icons";
import { useState } from 'react';
import {
  Server as ServerIcon
} from 'lucide-react';
import type { TreeNode, TreeNodeType } from '@/shared/models/database.types';
import type { Server } from '@/shared/models/servers.types';
import type { OpenTab } from '@/shared/hooks/use-query-tabs/use-query-tabs.types';
import { cn } from '@/lib/utils';

export const useTreeNode = (
  node: TreeNode,
  nodeId: string,
  childrenMap: Map<string, string[]>,
  openServerModal: (server: Server) => void,
  openNewTab: OpenTab,
) => {
  const [isMenuOpen, setIsMenuOpen] = useState<boolean>(false);
  const childrenIds = childrenMap.get(nodeId) || [];
  const hasChildren = node.hasChildren;
  const isExpanded = node.isExpanded;
  const metadata = node.metadata;

  const handleContextMenu = (e: React.MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setIsMenuOpen(true);
  };

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

    if (type === 'server' && serverId) {
      openNewTab(serverId, metadata.serverData?.default_database || '');
    }

    if (type !== 'server' && serverId) {
      openNewTab(serverId, metadata.databaseName);
    }

    setIsMenuOpen(false);
  };

  const getNodeIcon = (type: TreeNodeType, isConnected: boolean) => {
    switch (type) {
      case 'server':
        return (
          <ServerIcon className={cn('size-4', isConnected && 'text-primary')} />
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
    isMenuOpen,
    childrenIds,
    hasChildren,
    isExpanded,
    metadata,
    setIsMenuOpen,
    handleContextMenu,
    handleServerEdit,
    getNodeIcon,
    handleOpenNewTab,
  };
};
