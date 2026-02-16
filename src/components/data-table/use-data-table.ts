import { useCallback, useEffect, useMemo, useState } from 'react';

import type {
  EditableInfo,
  QueryColumnInfo,
} from '@/api/database/database-responses.types';
import type { CellChange, ChangesMap, DataTableRow } from './data-table.types';

export function useDataTable(
  rows: DataTableRow[],
  columns: QueryColumnInfo[],
  editableInfo?: EditableInfo | null,
) {
  const [changes, setChanges] = useState<ChangesMap>(new Map());

  const isEditable =
    !!editableInfo && editableInfo.primaryKeyColumns.length > 0;

  const pkColumnSet = useMemo(
    () => new Set(editableInfo?.primaryKeyColumns ?? []),
    [editableInfo],
  );

  // Reset changes when a new query is executed.
  // On pagination, columns reference stays the same so changes are preserved.
  useEffect(() => {
    setChanges(new Map());
  }, [columns]);

  const getRowPkKey = useCallback(
    (rowIndex: number): string | null => {
      if (!editableInfo) return null;
      const row = rows[rowIndex];
      if (!row) return null;

      return editableInfo.primaryKeyColumnIndices
        .map(idx => row[idx] ?? 'NULL')
        .join('|');
    },
    [rows, editableInfo],
  );

  const isColumnEditable = useCallback(
    (columnName: string): boolean => {
      if (!isEditable) return false;
      return !pkColumnSet.has(columnName);
    },
    [isEditable, pkColumnSet],
  );

  const updateCell = useCallback(
    (
      rowIndex: number,
      columnId: string,
      originalValue: string | null,
      newValue: string,
    ) => {
      const pkKey = getRowPkKey(rowIndex);
      if (!pkKey) return;

      setChanges(prev => {
        const next = new Map(prev);

        // If the new value matches the original, remove the change
        if (newValue === (originalValue ?? '')) {
          const rowChanges = next.get(pkKey);
          if (rowChanges) {
            const updated = new Map(rowChanges);
            updated.delete(columnId);
            if (updated.size === 0) {
              next.delete(pkKey);
            } else {
              next.set(pkKey, updated);
            }
          }
          return next;
        }

        const rowChanges = new Map(next.get(pkKey) ?? new Map());
        rowChanges.set(columnId, { columnId, originalValue, newValue });
        next.set(pkKey, rowChanges);
        return next;
      });
    },
    [getRowPkKey],
  );

  const isRowModified = useCallback(
    (rowIndex: number): boolean => {
      const pkKey = getRowPkKey(rowIndex);
      if (!pkKey) return false;
      return changes.has(pkKey);
    },
    [changes, getRowPkKey],
  );

  const isCellModified = useCallback(
    (rowIndex: number, columnId: string): boolean => {
      const pkKey = getRowPkKey(rowIndex);
      if (!pkKey) return false;
      return changes.get(pkKey)?.has(columnId) ?? false;
    },
    [changes, getRowPkKey],
  );

  const getCellDisplayValue = useCallback(
    (
      rowIndex: number,
      columnId: string,
      originalValue: string | null,
    ): string | null => {
      const pkKey = getRowPkKey(rowIndex);
      if (!pkKey) return originalValue;
      const change = changes.get(pkKey)?.get(columnId);
      return change ? change.newValue : originalValue;
    },
    [changes, getRowPkKey],
  );

  const getModifiedRows = useCallback(
    (): Array<{ pkKey: string; changes: Map<string, CellChange> }> => {
      return Array.from(changes.entries()).map(([pkKey, cellChanges]) => ({
        pkKey,
        changes: cellChanges,
      }));
    },
    [changes],
  );

  const discardChanges = useCallback(() => {
    setChanges(new Map());
  }, []);

  const changesCount = useMemo(() => {
    let count = 0;
    changes.forEach(rowChanges => {
      count += rowChanges.size;
    });
    return count;
  }, [changes]);

  return {
    isEditable,
    changes,
    changesCount,
    isColumnEditable,
    updateCell,
    isRowModified,
    isCellModified,
    getCellDisplayValue,
    getModifiedRows,
    discardChanges,
  };
}
