import type { TreeNode } from '@/shared/models/database.types';

export interface SearchTarget {
  serverId: number;
  serverName: string;
  databaseName: string;
  schemaName: string;
  tableName: string;
  searchTokens?: string[];
}

export interface GlobalSearchDialogProps {
  nodes: Map<string, TreeNode>;
  onOpenTable: (
    serverId: number,
    databaseName: string,
    schemaName: string,
    tableName: string,
  ) => Promise<void>;
}
