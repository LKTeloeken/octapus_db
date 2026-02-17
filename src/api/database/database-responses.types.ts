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

export interface EditableInfo {
  schema: string;
  table: string;
  primaryKeyColumns: string[];
  primaryKeyColumnIndices: number[];
}

export interface ExecuteQueryResponse {
  columns: QueryColumnInfo[];
  rows: (string | null)[][];
  hasMore: boolean;
  rowCount: number;
  totalCount: number | null;
  executionTimeMs: number;
  editableInfo: EditableInfo | null;
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
  dataType: string;
  isNullable: boolean;
  ordinal: number;
  defaultValue: string | null;
  isPrimaryKey: boolean;
  isForeignKey: boolean;
}
