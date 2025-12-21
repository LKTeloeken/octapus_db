import type { Server } from "./servers.types";

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
  column_default: string | null;
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
}

export type TreeNodeType =
  | "server"
  | "database"
  | "schema"
  | "table"
  | "column";

export type TreeNodeMetadata =
  | {
      type: "server";
      serverId: number;
      serverData?: Server;
    }
  | {
      type: "column";
      serverId: number;
      databaseName: string;
      dataType: string;
      isNullable: boolean;
      columnDefault: string | null;
    }
  | {
      type: Exclude<TreeNodeType, "column" | "server">;
      serverId: number;
      databaseName: string;
    };

export type QueryResultsRow = Record<string, unknown>;
