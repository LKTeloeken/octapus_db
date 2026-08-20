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

export type QueryMessageKind =
  | 'notice'
  | 'warning'
  | 'error'
  | 'info'
  | 'status';

/**
 * Mensagem que o banco emite fora do result set — no Postgres, os
 * `RAISE NOTICE/WARNING/INFO`, o erro detalhado e a linha de conclusão.
 * Chega em streaming pelo canal do comando, não dentro do QueryResult.
 */
export interface QueryMessage {
  kind: QueryMessageKind;
  /** Severidade crua do servidor (NOTICE, WARNING, ERROR, INFO, LOG, DEBUG) */
  severity: string;
  message: string;
  detail: string | null;
  hint: string | null;
  /** Pilha de chamadas do PL/pgSQL */
  context: string | null;
  sqlState: string | null;
  position: number | null;
  timestampMs: number;
}
