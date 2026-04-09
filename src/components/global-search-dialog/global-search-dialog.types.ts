import type { TreeNode } from '@/shared/models/database.types';

export interface SearchTarget {
  serverId: number;
  databaseName: string;
  schemaName: string;
  tableName: string;
  columnName?: string;
  type: 'table' | 'column';
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
