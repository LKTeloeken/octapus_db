import type { EditableInfo, QueryColumnInfo } from '@/api/types/query.types';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useShortcuts } from '@/lib/keybindings/use-shortcuts';
import type { CopiedCell, CopiedRows } from '@/stores/grid-clipboard-store';
import {
  columnsSignature,
  NO_PENDING_CHANGES,
  usePendingChangesStore,
  type PendingChanges,
} from '@/stores/pending-changes-store';
import { buildAddedRows } from './grid-clipboard';
import { ownsGridShortcut } from './keyboard-scope';
import type {
  ActivateCellFn,
  ActiveCell,
  CellPosition,
  DataTableRow,
  FocusCellFn,
  FocusedCell,
  GetCellDisplayValueFn,
  IsCellModifiedFn,
  IsColumnEditableFn,
  IsPrimaryKeyColumnFn,
  IsRowFlagFn,
  SaveRowChanges,
  SelectAllRowsFn,
  ToggleRowSelectionFn,
  UpdateCellFn,
  UseResultsTableReturn,
} from './results-table.types';
import {
  applyCellChange,
  buildSaveRowChanges,
  countCellChanges,
  pkKeyOf,
} from './pending-changes';

/** Stable identity for a row in the selection set, robust to load-more. */
const existingRowId = (pkKey: string) => `e:${pkKey}`;
const addedRowId = (tempId: string) => `a:${tempId}`;

/** Identidade estável para "nada selecionado", o caso comum */
const NO_ROWS: DataTableRow[] = [];
const NO_DEFAULTS: ReadonlySet<string> = new Set();

/** Cursor de teclado + origem do bloco de células (Shift+clique/setas, arraste) */
interface CellCursor {
  focused: FocusedCell;
  anchor: CellPosition | null;
}
const NO_CURSOR: CellCursor = { focused: null, anchor: null };

const useResultsTable = (
  rows: DataTableRow[],
  columns: QueryColumnInfo[],
  editableInfo: EditableInfo | null | undefined,
  onSave: (changes: SaveRowChanges) => void | Promise<void>,
  /** Dono das pendências na store: o id da aba (o `searchScopeId`) */
  scopeId: string,
  /** Região de teclado da grade (`keyboard-scope.ts`); `null` = a da aba */
  keyboardScope: string | null = null,
  /** Colunas com valor padrão: duplicar/colar na mesma grade as deixa vazias */
  defaultColumns: ReadonlySet<string> = NO_DEFAULTS,
): UseResultsTableReturn => {
  // As pendências (edições das linhas existentes, linhas novas e linhas
  // marcadas para remoção) moram na store, por aba: só a aba ativa fica
  // montada, e trocar de aba não pode perdê-las.
  const { changes, addedRows, removedRowKeys } =
    usePendingChangesStore(state => state.byScope.get(scopeId)) ??
    NO_PENDING_CHANGES;
  const updatePending = usePendingChangesStore(state => state.update);
  const clearPending = usePendingChangesStore(state => state.clear);
  const signature = useMemo(() => columnsSignature(columns), [columns]);
  const patchPending = useCallback(
    (recipe: (current: PendingChanges) => Partial<PendingChanges>) =>
      updatePending(scopeId, signature, recipe),
    [updatePending, scopeId, signature],
  );
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
  // moldura não abre editor nenhum. Com origem (`anchor`), cursor e origem
  // são os cantos de um bloco de células selecionado.
  const [cursor, setCursor] = useState<CellCursor>(NO_CURSOR);
  const focusedCell = cursor.focused;
  // A última seleção feita (linhas no gutter ou células) decide o que o
  // Cmd/Ctrl+C copia quando as duas estão na tela.
  const lastSelection = useRef<'rows' | 'cells'>('cells');

  const setFocusedCell = useCallback((cell: FocusedCell) => {
    lastSelection.current = 'cells';
    setCursor({ focused: cell, anchor: null });
  }, []);

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

  // A new query/table (columns change) invalidates every pending change and
  // the added rows (cells are positional). Remontar não é trocar de query: a
  // grade volta ao trocar de aba (ou a mesma instância passa a mostrar outra
  // aba) e as pendências guardadas valem se as colunas são as mesmas. Até lá,
  // `[]` é só o carregamento.
  const checkedColumns = useRef<{
    scopeId: string;
    columns: QueryColumnInfo[];
  } | null>(null);
  useEffect(() => {
    const checked =
      checkedColumns.current?.scopeId === scopeId
        ? checkedColumns.current.columns
        : null;
    if (checked === columns) return;
    if (checked === null && columns.length === 0) return;
    checkedColumns.current = { scopeId, columns };

    const stored = usePendingChangesStore.getState().byScope.get(scopeId);
    if (checked === null && stored?.columnsSignature === signature) return;
    clearPending(scopeId);
  }, [columns, signature, scopeId, clearPending]);

  // A seleção e a célula aberta são locais: outra grade começa sem elas.
  useEffect(() => {
    setSelectedRowIds(new Set());
    setSelectedColumns(new Set());
    setActiveCell(null);
    setCursor(NO_CURSOR);
    selectionAnchor.current = null;
  }, [columns, scopeId]);

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
    return rows.map(row => pkKeyOf(row, pkIndices));
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

  // As linhas selecionadas, na ordem da grade — é o que a exportação oferece
  // como alternativa ao resultado inteiro. Linhas adicionadas pendentes ficam
  // de fora: exporta-se o que está no banco, não o rascunho.
  const selectedRows = useMemo<DataTableRow[]>(() => {
    if (selectedRowIds.size === 0) return NO_ROWS;
    return rows.filter((_, index) => {
      const pkKey = rowKeys[index];
      return pkKey != null && selectedRowIds.has(existingRowId(pkKey));
    });
  }, [rows, rowKeys, selectedRowIds]);

  const updateCell = useCallback<UpdateCellFn>(
    (rowIndex, columnId, originalValue, newValue) => {
      // Edits to an added row are written straight into its positional cells.
      if (rowIndex >= existingCount) {
        const addedIndex = rowIndex - existingCount;
        const colIndex = columnIndexByName.get(columnId);
        if (colIndex === undefined) return;

        patchPending(({ addedRows: prev }) => {
          const target = prev[addedIndex];
          if (!target) return {};
          const next = [...prev];
          const cells = [...target.cells];
          cells[colIndex] = newValue;
          const touched = new Set(target.touched);
          touched.add(colIndex);
          next[addedIndex] = { ...target, cells, touched };
          return { addedRows: next };
        });
        return;
      }

      const pkKey = generateRowKey(rowIndex);
      if (!pkKey) return;

      patchPending(({ changes: prev }) => ({
        changes: applyCellChange(
          prev,
          pkKey,
          columnId,
          originalValue,
          newValue,
        ),
      }));
    },
    [existingCount, columnIndexByName, generateRowKey, patchPending],
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

  const isCellDefault = useCallback(
    (rowIndex: number, columnIndex: number) => {
      const added = addedRows[rowIndex - existingCount];
      return (
        rowIndex >= existingCount && !!added && !added.touched.has(columnIndex)
      );
    },
    [addedRows, existingCount],
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

  const focusCell = useCallback<FocusCellFn>(
    (rowIndex, columnName) => setFocusedCell({ rowIndex, columnName }),
    [setFocusedCell],
  );

  // Leva o cursor até a célula mantendo a origem: o bloco vai da primeira
  // célula (onde a seleção começou) até esta.
  const extendCellSelection = useCallback<FocusCellFn>(
    (rowIndex, columnName) => {
      lastSelection.current = 'cells';
      setCursor(prev => {
        const focused = { rowIndex, columnName };
        return { focused, anchor: prev.anchor ?? prev.focused ?? focused };
      });
    },
    [],
  );

  // ── Selection ──────────────────────────────────────────────────────────

  const toggleRowSelection = useCallback<ToggleRowSelectionFn>(
    (rowIndex, mode) => {
      const id = rowId(rowIndex);
      if (!id) return;

      // Snapshot the anchor now: the updater below runs during render, after
      // the `selectionAnchor.current = rowIndex` line would have mutated it.
      const anchor = selectionAnchor.current;
      lastSelection.current = 'rows';

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

  // Ids de todas as linhas selecionáveis da grade — sem PK não há identidade
  // estável, então essas linhas ficam fora da seleção (como no clique).
  const allRowIds = useMemo(() => {
    const ids: string[] = [];
    for (let i = 0; i < displayRows.length; i++) {
      const id = rowId(i);
      if (id) ids.push(id);
    }
    return ids;
  }, [displayRows.length, rowId]);

  const allRowsSelected =
    allRowIds.length > 0 &&
    selectedRowIds.size === allRowIds.length &&
    allRowIds.every(id => selectedRowIds.has(id));

  const selectAllRows = useCallback<SelectAllRowsFn>(() => {
    if (allRowIds.length === 0) return;
    lastSelection.current = 'rows';
    setSelectedRowIds(new Set(allRowIds));
    selectionAnchor.current = null;
  }, [allRowIds]);

  const toggleAllRowsSelection = useCallback<SelectAllRowsFn>(() => {
    if (allRowsSelected) {
      setSelectedRowIds(new Set());
      selectionAnchor.current = null;
      return;
    }
    selectAllRows();
  }, [allRowsSelected, selectAllRows]);

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
    patchPending(({ addedRows: prev }) => ({
      addedRows: [
        ...prev,
        { tempId, cells: columns.map(() => null), touched: new Set<number>() },
      ],
    }));
  }, [isEditable, columns, patchPending]);

  // Índices (na grade) das linhas selecionadas, em ordem — sob demanda, para
  // copiar e duplicar: com milhares de linhas não vale manter isso em memo.
  const selectedRowIndices = useCallback((): number[] => {
    if (selectedRowIds.size === 0) return [];
    const indices: number[] = [];
    for (let i = 0; i < displayRows.length; i++) {
      const id = rowId(i);
      if (id && selectedRowIds.has(id)) indices.push(i);
    }
    return indices;
  }, [selectedRowIds, displayRows.length, rowId]);

  // A linha como a grade mostra (com as edições pendentes), em todas as
  // colunas. Na linha nova, coluna não tocada sai `undefined` (default).
  const rowCells = useCallback(
    (rowIndex: number): CopiedCell[] => {
      const row = displayRows[rowIndex];
      if (!row) return [];
      const added =
        rowIndex >= existingCount ? addedRows[rowIndex - existingCount] : null;
      return columns.map((column, index) => {
        if (added) return added.touched.has(index) ? row[index] : undefined;
        return getCellDisplayValue(rowIndex, column.name, row[index] ?? null);
      });
    },
    [displayRows, existingCount, addedRows, columns, getCellDisplayValue],
  );

  const appendRows = useCallback(
    (
      source: { columns: string[]; rows: CopiedCell[][] },
      sameGrid: boolean,
    ) => {
      if (!isEditable) return 0;
      const added = buildAddedRows(
        source,
        columns,
        sameGrid ? defaultColumns : NO_DEFAULTS,
      );
      if (added.length > 0) {
        patchPending(({ addedRows: prev }) => ({
          addedRows: [...prev, ...added],
        }));
      }
      return added.length;
    },
    [isEditable, columns, defaultColumns, patchPending],
  );

  // Cmd/Ctrl+D: cópias pendentes das linhas selecionadas (ou da do cursor),
  // com as colunas de valor padrão (id serial, created_at…) vazias para o
  // banco preencher. Linha marcada para remoção não se duplica.
  const duplicateRows = useCallback(() => {
    const selected = selectedRowIndices();
    const sources = (
      selected.length > 0 ? selected : focusedCell ? [focusedCell.rowIndex] : []
    ).filter(index => index < displayRows.length && !isRowRemoved(index));
    return appendRows(
      { columns: columns.map(c => c.name), rows: sources.map(rowCells) },
      true,
    );
  }, [
    selectedRowIndices,
    focusedCell,
    displayRows.length,
    isRowRemoved,
    appendRows,
    columns,
    rowCells,
  ]);

  // Colar linhas copiadas (desta ou de outra aba) como linhas novas. Na mesma
  // grade é uma duplicação; noutra, os valores vão como estão (PK incluída —
  // copiar entre ambientes é o caso de uso).
  const pasteRows = useCallback(
    (copied: CopiedRows) => appendRows(copied, copied.scopeId === scopeId),
    [appendRows, scopeId],
  );

  const removeSelected = useCallback(() => {
    if (selectedRowIds.size === 0) return;

    const removedTempIds = new Set<string>();
    const removedPkKeys: string[] = [];

    selectedRowIds.forEach(id => {
      if (id.startsWith('a:')) removedTempIds.add(id.slice(2));
      else if (id.startsWith('e:')) removedPkKeys.push(id.slice(2));
    });

    if (removedTempIds.size > 0) {
      patchPending(({ addedRows: prev }) => ({
        addedRows: prev.filter(r => !removedTempIds.has(r.tempId)),
      }));
    }

    // Marking existing rows for deletion only matters when editable.
    if (isEditable && removedPkKeys.length > 0) {
      patchPending(({ removedRowKeys: prev }) => {
        const next = new Set(prev);
        removedPkKeys.forEach(key => next.add(key));
        return { removedRowKeys: next };
      });
    }

    clearSelection();
  }, [selectedRowIds, isEditable, clearSelection, patchPending]);

  // ── Persist ───────────────────────────────────────────────────────────

  const changesCount = useMemo(
    () => countCellChanges(changes, removedRowKeys),
    [changes, removedRowKeys],
  );

  const addedCount = addedRows.length;
  const removedCount = removedRowKeys.size;
  const pendingCount = changesCount + addedCount + removedCount;

  const save = useCallback(async () => {
    const { edits, inserts, deletes } = buildSaveRowChanges(
      { changes, addedRows, removedRowKeys },
      columns,
    );

    if (edits.length === 0 && inserts.length === 0 && deletes.length === 0) {
      return;
    }

    try {
      await onSave({ edits, inserts, deletes });
      // React Query keeps the columns reference stable across a refetch, so the
      // [columns] reset effect won't fire — clear explicitly once it succeeds.
      clearPending(scopeId);
      setActiveCell(null);
      // Deletar ou inserir linhas desloca os índices (a ordem padrão é PK
      // desc): o cursor voltaria para outra linha. Só edição mantém as
      // posições — e o painel de valor segue na célula recém-salva.
      if (deletes.length > 0 || inserts.length > 0) setFocusedCell(null);
      else setCursor(prev => ({ ...prev, anchor: null }));
      clearSelection();
    } catch {
      // Keep the pending changes highlighted so the user can retry.
    }
  }, [
    changes,
    removedRowKeys,
    addedRows,
    columns,
    onSave,
    clearPending,
    scopeId,
    clearSelection,
    setFocusedCell,
  ]);

  const discardChanges = useCallback(() => {
    clearPending(scopeId);
    setActiveCell(null);
    setFocusedCell(null);
    clearSelection();
  }, [clearPending, scopeId, clearSelection, setFocusedCell]);

  // Keyboard: Cmd/Ctrl+S saves, Backspace removes the selected rows, Esc
  // clears the selection. Only the active tab's table is mounted, so there is
  // never more than one listener competing. Num campo de texto só o salvar
  // vale — Cmd+Z/Backspace/Esc ficam com o campo (o desfazer nativo).
  useShortcuts(
    {
      'grid.save': () => {
        if (pendingCount > 0) void save();
      },
      'grid.selectAll': selectAllRows,
      'grid.discardChanges': () => {
        if (pendingCount > 0) discardChanges();
      },
      'grid.duplicateRows': () => {
        if (!isEditable) return false;
        duplicateRows();
      },
      'grid.deleteRows': () => {
        if (selectedRowIds.size === 0) return false;
        removeSelected();
      },
      'grid.clearSelection': () => {
        // Esc desfaz primeiro o bloco de células, depois a seleção de linhas
        if (cursor.anchor) {
          setCursor(prev => ({ ...prev, anchor: null }));
          return;
        }
        if (selectedRowIds.size === 0) return false;
        clearSelection();
      },
    },
    { when: event => ownsGridShortcut(event, keyboardScope) },
  );

  return {
    isEditable,
    isPrimaryKeyColumn,
    isColumnEditable,
    displayRows,
    updateCell,
    isRowModified,
    isCellModified,
    isCellDefault,
    isRowAdded,
    isRowRemoved,
    isRowSelected,
    isColumnSelected,
    selectedRows,
    getCellDisplayValue,
    activeCell,
    activateCell,
    deactivateCell,
    focusedCell,
    focusCell,
    setFocusedCell,
    cellAnchor: cursor.anchor,
    extendCellSelection,
    getLastSelection: () => lastSelection.current,
    selectedRowIndices,
    rowCells,
    duplicateRows,
    pasteRows,
    toggleRowSelection,
    selectAllRows,
    toggleAllRowsSelection,
    allRowsSelected,
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
