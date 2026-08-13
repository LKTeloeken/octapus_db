import type {
  EditableInfo,
  QueryColumnInfo,
  RowEdit,
  RowInsert,
} from '@/api/types/query.types';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type {
  ActivateCellFn,
  ActiveCell,
  AddedRow,
  ChangesMap,
  DataTableRow,
  FocusCellFn,
  FocusedCell,
  GetCellDisplayValueFn,
  IsCellModifiedFn,
  IsColumnEditableFn,
  IsPrimaryKeyColumnFn,
  IsRowFlagFn,
  RowSelectionMode,
  SaveRowChanges,
  ToggleRowSelectionFn,
  UpdateCellFn,
  UseResultsTableReturn,
} from './results-table.types';

/** Stable identity for a row in the selection set, robust to load-more. */
const existingRowId = (pkKey: string) => `e:${pkKey}`;
const addedRowId = (tempId: string) => `a:${tempId}`;

const useResultsTable = (
  rows: DataTableRow[],
  columns: QueryColumnInfo[],
  editableInfo: EditableInfo | null | undefined,
  onSave: (changes: SaveRowChanges) => void | Promise<void>,
): UseResultsTableReturn => {
  // Cell edits to EXISTING rows, keyed by pkKey → columnId → change.
  const [changes, setChanges] = useState<ChangesMap>(new Map());
  // Not-yet-persisted rows, rendered after the existing ones.
  const [addedRows, setAddedRows] = useState<AddedRow[]>([]);
  // pkKeys of existing rows marked for deletion (still shown, in red).
  const [removedRowKeys, setRemovedRowKeys] = useState<Set<string>>(new Set());
  // Selection (stable ids) for bulk remove; columns are visual-only.
  const [selectedRowIds, setSelectedRowIds] = useState<Set<string>>(new Set());
  const [selectedColumns, setSelectedColumns] = useState<Set<string>>(
    new Set(),
  );
  // Anchor (display index) for shift-range selection.
  const selectionAnchor = useRef<number | null>(null);
  // The single cell currently being edited/viewed (only one Popover mounts).
  const [activeCell, setActiveCell] = useState<ActiveCell>(null);
  // Cursor de navegação por teclado — separado da célula aberta: mover a
  // moldura não abre editor nenhum.
  const [focusedCell, setFocusedCell] = useState<FocusedCell>(null);

  const isEditable =
    !!editableInfo && editableInfo.primaryKeyColumns.length > 0;

  const primaryKeyColumnSet = useMemo(
    () => new Set(editableInfo?.primaryKeyColumns ?? []),
    [editableInfo],
  );

  const columnIndexByName = useMemo(() => {
    const map = new Map<string, number>();
    columns.forEach((c, i) => map.set(c.name, i));
    return map;
  }, [columns]);

  // A new query/table (columns change) invalidates every pending change,
  // the added rows (cells are positional) and the selection.
  useEffect(() => {
    setChanges(new Map());
    setAddedRows([]);
    setRemovedRowKeys(new Set());
    setSelectedRowIds(new Set());
    setSelectedColumns(new Set());
    setActiveCell(null);
    setFocusedCell(null);
    selectionAnchor.current = null;
  }, [columns]);

  const existingCount = rows.length;

  // No common case (sem linhas pendentes adicionadas) reusamos o array original
  // em vez de recopiá-lo a cada render — evita uma cópia O(n) em tabelas grandes.
  const displayRows = useMemo<DataTableRow[]>(
    () =>
      addedRows.length === 0 ? rows : [...rows, ...addedRows.map(r => r.cells)],
    [rows, addedRows],
  );

  const isPrimaryKeyColumn = useCallback<IsPrimaryKeyColumnFn>(
    columnName => primaryKeyColumnSet.has(columnName),
    [primaryKeyColumnSet],
  );

  const isColumnEditable = useCallback<IsColumnEditableFn>(
    columnName => {
      if (!isEditable) return false;
      return !primaryKeyColumnSet.has(columnName);
    },
    [isEditable, primaryKeyColumnSet],
  );

  // PK key per existing row, computed once per data change instead of being
  // re-joined on every cell render/scroll.
  const rowKeys = useMemo<(string | null)[]>(() => {
    const pkIndices = editableInfo?.primaryKeyColumnIndices;
    if (!pkIndices) return [];
    return rows.map(row => pkIndices.map(key => row[key] ?? 'NULL').join('|'));
  }, [rows, editableInfo]);

  const generateRowKey = useCallback(
    (rowIndex: number): string | null => rowKeys[rowIndex] ?? null,
    [rowKeys],
  );

  const isRowAdded = useCallback<IsRowFlagFn>(
    rowIndex => rowIndex >= existingCount,
    [existingCount],
  );

  // Stable selection id for a display row (existing → pkKey, added → tempId).
  const rowId = useCallback(
    (rowIndex: number): string | null => {
      if (rowIndex >= existingCount) {
        const added = addedRows[rowIndex - existingCount];
        return added ? addedRowId(added.tempId) : null;
      }
      const pkKey = generateRowKey(rowIndex);
      return pkKey ? existingRowId(pkKey) : null;
    },
    [existingCount, addedRows, generateRowKey],
  );

  const isRowRemoved = useCallback<IsRowFlagFn>(
    rowIndex => {
      if (rowIndex >= existingCount) return false;
      const pkKey = generateRowKey(rowIndex);
      return pkKey ? removedRowKeys.has(pkKey) : false;
    },
    [existingCount, generateRowKey, removedRowKeys],
  );

  const isRowSelected = useCallback<IsRowFlagFn>(
    rowIndex => {
      const id = rowId(rowIndex);
      return id ? selectedRowIds.has(id) : false;
    },
    [rowId, selectedRowIds],
  );

  const isColumnSelected = useCallback(
    (columnName: string) => selectedColumns.has(columnName),
    [selectedColumns],
  );

  const updateCell = useCallback<UpdateCellFn>(
    (rowIndex, columnId, originalValue, newValue) => {
      // Edits to an added row are written straight into its positional cells.
      if (rowIndex >= existingCount) {
        const addedIndex = rowIndex - existingCount;
        const colIndex = columnIndexByName.get(columnId);
        if (colIndex === undefined) return;

        setAddedRows(prev => {
          const next = [...prev];
          const target = next[addedIndex];
          if (!target) return prev;
          const cells = [...target.cells];
          cells[colIndex] = newValue;
          const touched = new Set(target.touched);
          touched.add(colIndex);
          next[addedIndex] = { ...target, cells, touched };
          return next;
        });
        return;
      }

      const pkKey = generateRowKey(rowIndex);
      if (!pkKey) return;

      setChanges(prev => {
        const newChanges = new Map(prev);
        let rowChanges = newChanges.get(pkKey);

        // Comparação null-aware: '' e NULL são valores distintos.
        if (newValue === originalValue) {
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
    [existingCount, columnIndexByName, generateRowKey],
  );

  const isRowModified = useCallback<IsRowFlagFn>(
    rowIndex => {
      if (rowIndex >= existingCount) return false;
      const pkKey = generateRowKey(rowIndex);
      if (!pkKey) return false;
      return changes.has(pkKey);
    },
    [existingCount, changes, generateRowKey],
  );

  const isCellModified = useCallback<IsCellModifiedFn>(
    (rowIndex, columnId) => {
      if (rowIndex >= existingCount) return false;
      const pkKey = generateRowKey(rowIndex);
      if (!pkKey) return false;
      return changes.get(pkKey)?.has(columnId) ?? false;
    },
    [existingCount, changes, generateRowKey],
  );

  const getCellDisplayValue = useCallback<GetCellDisplayValueFn>(
    (rowIndex, columnId, originalValue) => {
      // Added rows hold their value directly in the passed cell.
      if (rowIndex >= existingCount) return originalValue;

      const pkKey = generateRowKey(rowIndex);
      if (!pkKey) return originalValue;

      const change = changes.get(pkKey)?.get(columnId);
      return change ? change.newValue : originalValue;
    },
    [existingCount, changes, generateRowKey],
  );

  // ── Active editor (single Popover) ───────────────────────────────────────

  const activateCell = useCallback<ActivateCellFn>((rowIndex, columnName) => {
    setActiveCell({ rowIndex, columnName });
  }, []);

  const deactivateCell = useCallback(() => setActiveCell(null), []);

  const focusCell = useCallback<FocusCellFn>((rowIndex, columnName) => {
    setFocusedCell({ rowIndex, columnName });
  }, []);

  // ── Selection ──────────────────────────────────────────────────────────

  const toggleRowSelection = useCallback<ToggleRowSelectionFn>(
    (rowIndex, mode) => {
      const id = rowId(rowIndex);
      if (!id) return;

      // Snapshot the anchor now: the updater below runs during render, after
      // the `selectionAnchor.current = rowIndex` line would have mutated it.
      const anchor = selectionAnchor.current;

      setSelectedRowIds(prev => {
        if (mode === 'range' && anchor !== null) {
          const next = new Set(prev);
          const start = Math.min(anchor, rowIndex);
          const end = Math.max(anchor, rowIndex);
          for (let i = start; i <= end; i++) {
            const rid = rowId(i);
            if (rid) next.add(rid);
          }
          return next;
        }

        if (mode === 'toggle') {
          const next = new Set(prev);
          if (next.has(id)) next.delete(id);
          else next.add(id);
          return next;
        }

        // single: replace selection, unless it's the only selected row
        if (prev.size === 1 && prev.has(id)) return new Set();
        return new Set([id]);
      });

      // Keep the anchor on a range extend so further Shift-clicks grow from the
      // same origin; otherwise the clicked row becomes the new anchor.
      if (mode !== 'range') {
        selectionAnchor.current = rowIndex;
      }
    },
    [rowId],
  );

  const toggleColumnSelection = useCallback((columnName: string) => {
    setSelectedColumns(prev => {
      const next = new Set(prev);
      if (next.has(columnName)) next.delete(columnName);
      else next.add(columnName);
      return next;
    });
  }, []);

  const clearSelection = useCallback(() => {
    setSelectedRowIds(new Set());
    setSelectedColumns(new Set());
    selectionAnchor.current = null;
  }, []);

  // ── Add / remove rows ──────────────────────────────────────────────────

  const addRow = useCallback(() => {
    if (!isEditable) return;
    const tempId =
      typeof crypto !== 'undefined' && 'randomUUID' in crypto
        ? crypto.randomUUID()
        : `${Date.now()}-${Math.random()}`;
    setAddedRows(prev => [
      ...prev,
      { tempId, cells: columns.map(() => null), touched: new Set<number>() },
    ]);
  }, [isEditable, columns]);

  const removeSelected = useCallback(() => {
    if (selectedRowIds.size === 0) return;

    const removedTempIds = new Set<string>();
    const removedPkKeys: string[] = [];

    selectedRowIds.forEach(id => {
      if (id.startsWith('a:')) removedTempIds.add(id.slice(2));
      else if (id.startsWith('e:')) removedPkKeys.push(id.slice(2));
    });

    if (removedTempIds.size > 0) {
      setAddedRows(prev => prev.filter(r => !removedTempIds.has(r.tempId)));
    }

    // Marking existing rows for deletion only matters when editable.
    if (isEditable && removedPkKeys.length > 0) {
      setRemovedRowKeys(prev => {
        const next = new Set(prev);
        removedPkKeys.forEach(key => next.add(key));
        return next;
      });
    }

    clearSelection();
  }, [selectedRowIds, isEditable, clearSelection]);

  // ── Persist ───────────────────────────────────────────────────────────

  const buildEdits = useCallback((): RowEdit[] => {
    const result: RowEdit[] = [];

    changes.forEach((rowChanges, pkKey) => {
      // Skip rows that are being deleted in the same save.
      if (removedRowKeys.has(pkKey)) return;

      const pkValues = pkKey.split('|').map(v => (v === 'NULL' ? null : v));
      const changesArray: [string, string | null][] = [];
      rowChanges.forEach(cellChange => {
        changesArray.push([cellChange.columnId, cellChange.newValue]);
      });

      result.push({ pkValues, changes: changesArray });
    });

    return result;
  }, [changes, removedRowKeys]);

  const buildInserts = useCallback((): RowInsert[] => {
    return addedRows
      .map(row => {
        const values: [string, string | null][] = [];
        row.cells.forEach((value, i) => {
          // Coluna não tocada → o banco aplica o default; tocada entra no
          // INSERT mesmo com valor null (NULL explícito).
          if (row.touched.has(i)) values.push([columns[i].name, value]);
        });
        return { values };
      })
      .filter(insert => insert.values.length > 0);
  }, [addedRows, columns]);

  const buildDeletes = useCallback(
    (): (string | null)[][] =>
      Array.from(removedRowKeys).map(key =>
        key.split('|').map(v => (v === 'NULL' ? null : v)),
      ),
    [removedRowKeys],
  );

  const changesCount = useMemo(() => {
    let count = 0;
    changes.forEach((rowChanges, pkKey) => {
      if (removedRowKeys.has(pkKey)) return;
      count += rowChanges.size;
    });
    return count;
  }, [changes, removedRowKeys]);

  const addedCount = addedRows.length;
  const removedCount = removedRowKeys.size;
  const pendingCount = changesCount + addedCount + removedCount;

  const save = useCallback(async () => {
    const edits = buildEdits();
    const inserts = buildInserts();
    const deletes = buildDeletes();

    if (edits.length === 0 && inserts.length === 0 && deletes.length === 0) {
      return;
    }

    try {
      await onSave({ edits, inserts, deletes });
      // React Query keeps the columns reference stable across a refetch, so the
      // [columns] reset effect won't fire — clear explicitly once it succeeds.
      setChanges(new Map());
      setAddedRows([]);
      setRemovedRowKeys(new Set());
      setActiveCell(null);
      // Deletar linhas desloca os índices: o cursor voltaria para outra linha.
      setFocusedCell(null);
      clearSelection();
    } catch {
      // Keep the pending changes highlighted so the user can retry.
    }
  }, [buildEdits, buildInserts, buildDeletes, onSave, clearSelection]);

  const discardChanges = useCallback(() => {
    setChanges(new Map());
    setAddedRows([]);
    setRemovedRowKeys(new Set());
    setActiveCell(null);
    setFocusedCell(null);
    clearSelection();
  }, [clearSelection]);

  // Keyboard: Cmd/Ctrl+S saves, Backspace removes the selected rows, Esc
  // clears the selection. Only the active tab's table is mounted, so there is
  // never more than one listener competing.
  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 's') {
        event.preventDefault();
        if (pendingCount > 0) void save();
        return;
      }

      // Don't hijack Cmd+Z/Backspace/Esc while typing in an editor — deixa o
      // desfazer nativo do campo funcionar.
      const target = event.target as HTMLElement | null;
      const isTyping =
        !!target &&
        (target.tagName === 'INPUT' ||
          target.tagName === 'TEXTAREA' ||
          target.isContentEditable);
      if (isTyping) return;

      // Cmd/Ctrl+Z descarta todas as alterações pendentes.
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'z') {
        event.preventDefault();
        if (pendingCount > 0) discardChanges();
        return;
      }

      if (
        (event.key === 'Backspace' || event.key === 'Delete') &&
        selectedRowIds.size > 0
      ) {
        event.preventDefault();
        removeSelected();
        return;
      }

      if (event.key === 'Escape' && selectedRowIds.size > 0) {
        clearSelection();
      }
    };

    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [
    pendingCount,
    save,
    discardChanges,
    selectedRowIds,
    removeSelected,
    clearSelection,
  ]);

  return {
    isEditable,
    isPrimaryKeyColumn,
    isColumnEditable,
    displayRows,
    updateCell,
    isRowModified,
    isCellModified,
    isRowAdded,
    isRowRemoved,
    isRowSelected,
    isColumnSelected,
    getCellDisplayValue,
    activeCell,
    activateCell,
    deactivateCell,
    focusedCell,
    focusCell,
    setFocusedCell,
    toggleRowSelection,
    toggleColumnSelection,
    clearSelection,
    addRow,
    removeSelected,
    discardChanges,
    save,
    changesCount,
    addedCount,
    removedCount,
    pendingCount,
  };
};

export default useResultsTable;
