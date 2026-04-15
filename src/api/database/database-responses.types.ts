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
  foreignKeyTarget?: ForeignKeyTarget | null;
}

export interface EditableInfo {
  schema: string;
  table: string;
  primaryKeyColumns: string[];
  primaryKeyColumnIndices: number[];
}

export interface RowEdit {
  /** Primary key values in the same order as EditableInfo.primaryKeyColumns */
  pkValues: (string | null)[];
  /** (columnName, newValue) pairs for each changed cell */
  changes: [string, string | null][];
}

export interface QueryChangeSet {
  edits: RowEdit[];
  insertedRows: (string | null)[][];
  deletedRowsPkValues: (string | null)[][];
  insertColumnNames: string[];
}

export interface ExecuteQueryResponse {
  columns: QueryColumnInfo[];
  rows: (string | null)[][];
  hasMore: boolean;
  rowCount: number;
  totalCount: number | null;
  executionTimeMs: number;
  editableInfo: EditableInfo | null;
  queryId: string | null;
}

export interface ApplyRowEditsResponse {
  affectedRows: number;
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
  dataType: string;
  isNullable: boolean;
  ordinal: number;
  defaultValue: string | null;
  isPrimaryKey: boolean;
  isForeignKey: boolean;
  foreignKeyTarget?: ForeignKeyTarget | null;
}

export interface ForeignKeyTarget {
  schema: string;
  table: string;
  column: string;
}
