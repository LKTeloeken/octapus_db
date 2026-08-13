import { call } from './client';
import { RustCommand } from './commands';
import type {
  EditableInfo,
  QueryOptions,
  QueryResult,
  RowEdit,
  RowInsert,
  StatementResult,
} from './types/query.types';

export function executeQuery(
  serverId: number,
  database: string,
  query: string,
  options?: QueryOptions,
): Promise<QueryResult> {
  return call<QueryResult>(RustCommand.ExecuteQuery, {
    serverId,
    database,
    query,
    options,
  });
}

export function executeStatement(
  serverId: number,
  database: string,
  statement: string,
): Promise<StatementResult> {
  return call<StatementResult>(RustCommand.ExecuteStatement, {
    serverId,
    database,
    statement,
  });
}

export function executeTransaction(
  serverId: number,
  database: string,
  statements: string[],
): Promise<StatementResult[]> {
  return call<StatementResult[]>(RustCommand.ExecuteTransaction, {
    serverId,
    database,
    statements,
  });
}

export function applyRowEdits(
  serverId: number,
  database: string,
  editable: EditableInfo,
  edits: RowEdit[],
): Promise<StatementResult> {
  return call<StatementResult>(RustCommand.ApplyRowEdits, {
    serverId,
    database,
    editable,
    edits,
  });
}

export function insertRows(
  serverId: number,
  database: string,
  editable: EditableInfo,
  rows: RowInsert[],
): Promise<StatementResult> {
  return call<StatementResult>(RustCommand.InsertRows, {
    serverId,
    database,
    editable,
    rows,
  });
}

export function deleteRows(
  serverId: number,
  database: string,
  editable: EditableInfo,
  pkValues: (string | null)[][],
): Promise<StatementResult> {
  return call<StatementResult>(RustCommand.DeleteRows, {
    serverId,
    database,
    editable,
    pkValues,
  });
}

export function cancelQuery(
  serverId: number,
  database: string,
  queryId: string,
): Promise<void> {
  return call<void>(RustCommand.CancelQuery, { serverId, database, queryId });
}
