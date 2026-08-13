export interface QueryColumnInfo {
  name: string;
  typeName: string;
  /** Postgres only */
  typeOid: number | null;
}

export interface EditableInfo {
  schema: string;
  table: string;
  primaryKeyColumns: string[];
  /** Position of each PK in columns/rows */
  primaryKeyColumnIndices: number[];
}

export interface RowEdit {
  /** Primary key values in the same order as EditableInfo.primaryKeyColumns */
  pkValues: (string | null)[];
  /** (columnName, newValue) pairs for each changed cell */
  changes: [string, string | null][];
}

export interface RowInsert {
  /** (columnName, value) pairs for the filled columns only; omitted columns
   *  fall back to the database default (serial, DEFAULT, generated _id) */
  values: [string, string | null][];
}

export interface QueryResult {
  columns: QueryColumnInfo[];
  /** Every cell is string or null — formatting is the front's job */
  rows: (string | null)[][];
  /** Number of rows in THIS page */
  rowCount: number;
  /** Total without pagination — only when countTotal=true */
  totalCount: number | null;
  hasMore: boolean;
  executionTimeMs: number;
  /** Non-null → rows can be edited inline */
  editableInfo: EditableInfo | null;
}

export interface QueryOptions {
  limit?: number;
  offset?: number;
  countTotal?: boolean;
  unlimited?: boolean;
}

export interface StatementResult {
  affectedRows: number;
  executionTimeMs: number;
}
