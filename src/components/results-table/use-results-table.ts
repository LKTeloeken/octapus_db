import type {
  EditableInfo,
  QueryColumnInfo,
} from '@/api/database/database-responses.types';
import { useCallback, useEffect, useMemo, useState } from 'react';
import type {
  ChangesMap,
  DataTableRow,
  GetCellDisplayValueFn,
  GetModifiedRowsFn,
  IsCellModifiedFn,
  IsColumnEditableFn,
  IsPrimaryKeyColumnFn,
  IsRowModifiedFn,
  UpdateCellFn,
  UseResultsTableReturn,
} from './results-table.types';

const useResultsTable = (
  rows: DataTableRow[],
  columns: QueryColumnInfo[],
  editableInfo: EditableInfo | null | undefined,
): UseResultsTableReturn => {
  const [changes, setChanges] = useState<ChangesMap>(new Map());

  const isEditable =
    !!editableInfo && editableInfo.primaryKeyColumns.length > 0;

  // Set to store the primary key column names
  const primaryKeyColumnSet = useMemo(
    () => new Set(editableInfo?.primaryKeyColumns ?? []),
    [editableInfo],
  );

  useEffect(() => {
    setChanges(new Map());
  }, [columns]);

  const isPrimaryKeyColumn = useCallback<IsPrimaryKeyColumnFn>(
    columnName => {
      return editableInfo?.primaryKeyColumns.includes(columnName) ?? false;
    },
    [editableInfo],
  );

  const isColumnEditable = useCallback<IsColumnEditableFn>(
    columnName => {
      if (!isEditable) return false;
      return !primaryKeyColumnSet.has(columnName);
    },
    [isEditable, primaryKeyColumnSet],
  );

  const generateRowKey = useCallback(
    (rowIndex: number): string | null => {
      if (!editableInfo) return null;
      const row = rows[rowIndex];
      if (!row) return null;

      const { primaryKeyColumnIndices } = editableInfo;
      if (!primaryKeyColumnIndices) return null;

      // Generate a unique key for the row based on the primary key row values
      return primaryKeyColumnIndices.map(key => row[key] ?? 'NULL').join('|');
    },
    [editableInfo, rows],
  );

  const updateCell = useCallback<UpdateCellFn>(
    (rowIndex, columnId, originalValue, newValue) => {
      const pkKey = generateRowKey(rowIndex);
      if (!pkKey) return;

      setChanges(prev => {
        const newChanges = new Map(prev);
        let rowChanges = newChanges.get(pkKey);

        if (newValue === (originalValue ?? '')) {
          if (!rowChanges) return newChanges;

          rowChanges.delete(columnId);

          if (rowChanges.size === 0) {
            newChanges.delete(pkKey);
          } else {
            newChanges.set(pkKey, rowChanges);
          }

          return newChanges;
        }

        rowChanges ??= new Map();
        rowChanges.set(columnId, { columnId, originalValue, newValue });
        newChanges.set(pkKey, rowChanges);
        return newChanges;
      });
    },
    [generateRowKey],
  );

  const isRowModified = useCallback<IsRowModifiedFn>(
    rowIndex => {
      const pkKey = generateRowKey(rowIndex);
      if (!pkKey) return false;
      return changes.has(pkKey);
    },
    [changes, generateRowKey],
  );

  const isCellModified = useCallback<IsCellModifiedFn>(
    (rowIndex, columnId) => {
      const pkKey = generateRowKey(rowIndex);
      if (!pkKey) return false;
      return changes.get(pkKey)?.has(columnId) ?? false;
    },
    [changes, generateRowKey],
  );

  const getCellDisplayValue = useCallback<GetCellDisplayValueFn>(
    (rowIndex, columnId, originalValue) => {
      const pkKey = generateRowKey(rowIndex);
      if (!pkKey) return originalValue;

      const change = changes.get(pkKey)?.get(columnId);
      return change ? change.newValue : originalValue;
    },
    [changes, generateRowKey],
  );

  const getModifiedRows = useCallback<GetModifiedRowsFn>(() => {
    return Array.from(changes.entries()).map(([pkKey, cellChanges]) => ({
      pkKey,
      changes: cellChanges,
    }));
  }, [changes]);

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
    isPrimaryKeyColumn,
    isColumnEditable,
    updateCell,
    isRowModified,
    isCellModified,
    getCellDisplayValue,
    getModifiedRows,
    discardChanges,
    changesCount,
  };
};

export default useResultsTable;
