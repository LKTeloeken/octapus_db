import type {
  EditableInfo,
  QueryColumnInfo,
  RowEdit,
  RowInsert,
} from '@/api/types/query.types';
import type { SortSpec } from '@/api/types/browse.types';

export type DataTableRow = (string | null)[];

/** Horizontal grid (default) vs transposed record view (fields as rows) */
export type ResultsViewMode = 'table' | 'vertical';

/** Pending row changes flushed together when the user saves */
export interface SaveRowChanges {
  edits: RowEdit[];
  inserts: RowInsert[];
  /** PK tuples (EditableInfo.primaryKeyColumns order) of rows to delete */
  deletes: (string | null)[][];
}

/** A not-yet-persisted row; cells are aligned to `columns` by index */
export interface AddedRow {
  tempId: string;
  cells: (string | null)[];
  /** Índices de colunas editadas pelo usuário: distingue "não tocada" (o banco
   *  aplica o default) de NULL explícito */
  touched: Set<number>;
}

/** How a click on a row handle mutates the current selection */
export type RowSelectionMode = 'single' | 'toggle' | 'range';

/** Endereço de uma célula na grade */
export interface CellPosition {
  rowIndex: number;
  columnName: string;
}

/** The single cell currently open for editing/viewing, or null */
export type ActiveCell = CellPosition | null;
/** Célula sob o cursor do teclado (navegação), independente da que está aberta */
export type FocusedCell = CellPosition | null;
export type ActivateCellFn = (rowIndex: number, columnName: string) => void;
export type FocusCellFn = (rowIndex: number, columnName: string) => void;

export interface ResultsTableProps {
  columns: QueryColumnInfo[];
  rows: DataTableRow[];
  /** Sort by a single column (header click); toggles asc↔desc */
  onSort: (column: string) => void;
  onLoadMore: () => void;
  /** Resolve to persist; reject to keep the pending (highlighted) changes */
  onSave: (changes: SaveRowChanges) => void | Promise<void>;
  /** The column/direction currently ordering the data (incl. the PK default) */
  activeSort?: SortSpec | null;
  editableInfo?: EditableInfo | null;
  /** Nomes de colunas ocultas no front (só renderização; índices e edição
   *  continuam usando o array `columns` completo) */
  hiddenColumns?: Set<string>;
  isLoading?: boolean;
  isLoadingMore?: boolean;
  hasMore?: boolean;
  executionTimeMs?: number;
  totalCount?: number | null;
  rowCount?: number;
  /** Message shown when there are no rows */
  emptyMessage?: string;
  className?: string;
}

export interface CellChange {
  columnId: string;
  originalValue: string | null;
  newValue: string | null;
}

export type ChangesMap = Map<string, Map<string, CellChange>>;

// useResultsTable function types
export type IsPrimaryKeyColumnFn = (columnName: string) => boolean;
export type IsColumnEditableFn = (columnName: string) => boolean;
export type UpdateCellFn = (
  rowIndex: number,
  columnId: string,
  originalValue: string | null,
  newValue: string | null,
) => void;
export type IsRowFlagFn = (rowIndex: number) => boolean;
export type IsRowModifiedFn = IsRowFlagFn;
export type IsCellModifiedFn = (rowIndex: number, columnId: string) => boolean;
export type GetCellDisplayValueFn = (
  rowIndex: number,
  columnId: string,
  originalValue: string | null,
) => string | null;
export type DiscardChangesFn = () => void;
export type SaveFn = () => void;
export type AddRowFn = () => void;
export type RemoveSelectedFn = () => void;
export type ClearSelectionFn = () => void;
export type ToggleRowSelectionFn = (
  rowIndex: number,
  mode: RowSelectionMode,
) => void;
export type ToggleColumnSelectionFn = (columnName: string) => void;

export interface UseResultsTableParams {
  rows: DataTableRow[];
  columns: QueryColumnInfo[];
  editableInfo: EditableInfo | null | undefined;
}

export interface UseResultsTableReturn {
  isEditable: boolean;
  isPrimaryKeyColumn: IsPrimaryKeyColumnFn;
  isColumnEditable: IsColumnEditableFn;
  /** Existing rows + pending added rows, ready for the virtualizer */
  displayRows: DataTableRow[];
  updateCell: UpdateCellFn;
  isRowModified: IsRowModifiedFn;
  isCellModified: IsCellModifiedFn;
  isRowAdded: IsRowFlagFn;
  isRowRemoved: IsRowFlagFn;
  isRowSelected: IsRowFlagFn;
  isColumnSelected: (columnName: string) => boolean;
  getCellDisplayValue: GetCellDisplayValueFn;
  /** The single cell open for editing/viewing, or null */
  activeCell: ActiveCell;
  activateCell: ActivateCellFn;
  deactivateCell: () => void;
  /** Célula sob o cursor do teclado (moldura), ou null */
  focusedCell: FocusedCell;
  focusCell: FocusCellFn;
  setFocusedCell: (cell: FocusedCell) => void;
  toggleRowSelection: ToggleRowSelectionFn;
  toggleColumnSelection: ToggleColumnSelectionFn;
  clearSelection: ClearSelectionFn;
  addRow: AddRowFn;
  removeSelected: RemoveSelectedFn;
  discardChanges: DiscardChangesFn;
  save: SaveFn;
  changesCount: number;
  addedCount: number;
  removedCount: number;
  pendingCount: number;
}
