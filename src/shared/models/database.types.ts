import type { Server } from './servers.types';
import { TableTypes } from '@/api/database/database-responses.types';

// All Database Structures
export interface ColumnStructure {
  name: string;
  data_type: string;
  is_nullable: boolean;
}

export interface TableStructure {
  name: string;
  tableType: TableTypes;
  columns?: ColumnStructure[];
}

export interface SchemaStructure {
  name: string;
  tables: TableStructure[];
}

export interface DatabaseStructure {
  schemas: SchemaStructure[];
}

// Individual Database Entities
export interface Database {
  name: string;
  serverId: number;
}

export interface Schema {
  name: string;
}

export interface Table {
  name: string;
}

export interface Column {
  name: string;
  data_type: string;
  is_nullable: boolean;
  default_value: string | null;
}

export interface TreeNode {
  id: string;
  type: TreeNodeType;
  name: string;
  parentId: string | null;
  hasChildren: boolean;
  isExpanded: boolean;
  isLoading: boolean;
  metadata: TreeNodeMetadata;
  isConnected?: boolean;
  hasLoadedChildren?: boolean;
}

export enum TreeNodeType {
  Server = 'server',
  Database = 'database',
  Schema = 'schema',
  Table = 'table',
  Column = 'column',
}

export type TreeNodeMetadata =
  | {
      type: 'server';
      serverId: number;
      serverData?: Server;
    }
  | {
      type: 'column';
      serverId: number;
      databaseName: string;
      dataType: string;
      isNullable: boolean;
      columnDefault: string | null;
    }
  | {
      type: Exclude<TreeNodeType, 'column' | 'server'>;
      serverId: number;
      databaseName: string;
    };

export type QueryResultsRow = string[][];
