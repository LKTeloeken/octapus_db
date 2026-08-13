import { useMutation, useQueryClient } from '@tanstack/react-query';
import { applyRowEdits, deleteRows, insertRows } from '@/api/query';
import type {
  EditableInfo,
  RowEdit,
  RowInsert,
} from '@/api/types/query.types';
import { queryKeys } from './keys';

export interface ApplyRowEditsVariables {
  serverId: number;
  database: string;
  editable: EditableInfo;
  edits: RowEdit[];
}

export interface InsertRowsVariables {
  serverId: number;
  database: string;
  editable: EditableInfo;
  rows: RowInsert[];
}

export interface DeleteRowsVariables {
  serverId: number;
  database: string;
  editable: EditableInfo;
  pkValues: (string | null)[][];
}

/** Persists inline cell edits and refreshes every browse view of that table */
export function useApplyRowEdits() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ serverId, database, editable, edits }: ApplyRowEditsVariables) =>
      applyRowEdits(serverId, database, editable, edits),
    onSuccess: (_result, { serverId, database, editable }) => {
      queryClient.invalidateQueries({
        queryKey: queryKeys.tableDataForTable(
          serverId,
          database,
          editable.schema || null,
          editable.table,
        ),
      });
    },
  });
}

/** Inserts new rows and refreshes every browse view of that table */
export function useInsertRows() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ serverId, database, editable, rows }: InsertRowsVariables) =>
      insertRows(serverId, database, editable, rows),
    onSuccess: (_result, { serverId, database, editable }) => {
      queryClient.invalidateQueries({
        queryKey: queryKeys.tableDataForTable(
          serverId,
          database,
          editable.schema || null,
          editable.table,
        ),
      });
    },
  });
}

/** Deletes rows by primary key and refreshes every browse view of that table */
export function useDeleteRows() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ serverId, database, editable, pkValues }: DeleteRowsVariables) =>
      deleteRows(serverId, database, editable, pkValues),
    onSuccess: (_result, { serverId, database, editable }) => {
      queryClient.invalidateQueries({
        queryKey: queryKeys.tableDataForTable(
          serverId,
          database,
          editable.schema || null,
          editable.table,
        ),
      });
    },
  });
}
