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
  dataType: string;
  isNullable: boolean;
  columnDefault: string | null;
}

export interface TreeNode {
  id: string;
  type: TreeNodeType;
  name: string;
  parentId: string | null;
  hasChildren: boolean;
  isExpanded: boolean;
  isLoading: boolean;
  metadata?: Record<string, any>; // type info, constraints, etc.
}

export type TreeNodeType =
  | "server"
  | "database"
  | "schema"
  | "table"
  | "column";
