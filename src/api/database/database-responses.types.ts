export interface GetDatabasesResponse {
  name: string;
  sizeBytes: number;
}

export interface GetSchemasWithTablesResponse {
  fetchedAt: number;
  schemas: SchemaWithTable[];
}

export type GetColumnsResponse = Column[];

export interface QueryColumnInfo {
  name: string;
  typeName: string;
  typeOid: number | null;
}

export interface ExecuteQueryResponse {
  columns: QueryColumnInfo[];
  rows: (string | null)[][];
  hasMore: boolean;
  rowCount: number;
  totalCount: number | null;
  executionTimeMs: number;
}

export enum TableTypes {
  Table = 'table',
  View = 'view',
  MaterializedView = 'materializedView',
  Foreign = 'foreign',
}

interface SchemaWithTable {
  name: string;
  tables: Table[];
}

interface Table {
  name: string;
  tableType: TableTypes;
}

interface Column {
  name: string;
  data_type: string;
  is_nullable: boolean;
  ordinal: number;
  default_value: string | null;
  is_primary_key: boolean;
  is_foreign_key: boolean;
}
