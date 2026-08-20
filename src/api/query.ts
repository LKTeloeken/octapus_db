import { Channel } from '@tauri-apps/api/core';
import { call } from './client';
import { RustCommand } from './commands';
import type {
  EditableInfo,
  QueryMessage,
  QueryOptions,
  QueryResult,
  RowEdit,
  RowInsert,
  StatementResult,
} from './types/query.types';

/**
 * `onMessage` liga o canal de mensagens do backend: cada RAISE, o erro
 * detalhado e a linha de conclusão chegam por ele enquanto a query roda.
 * Sem ele nenhum canal é criado e o backend não emite nada.
 */
export function executeQuery(
  serverId: number,
  database: string,
  query: string,
  options?: QueryOptions,
  onMessage?: (message: QueryMessage) => void,
): Promise<QueryResult> {
  let messages: Channel<QueryMessage> | undefined;

  if (onMessage) {
    messages = new Channel<QueryMessage>();
    messages.onmessage = onMessage;
  }

  return call<QueryResult>(RustCommand.ExecuteQuery, {
    serverId,
    database,
    query,
    options,
    messages,
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
