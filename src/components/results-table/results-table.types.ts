import type {
  EditableInfo,
  QueryColumnInfo,
} from '@/api/database/database-responses.types';

export type DataTableRow = (string | null)[];

export interface ResultsTableProps {
  columns: QueryColumnInfo[];
  rows: DataTableRow[];
  onLoadMore: () => void;
  editableInfo?: EditableInfo | null;
  isLoading?: boolean;
  isLoadingMore?: boolean;
  hasMore?: boolean;
  executionTimeMs?: number;
  totalCount?: number | null;
  rowCount?: number;
  className?: string;
}

export interface CellChange {
  columnId: string;
  originalValue: string | null;
  newValue: string;
}

export type ChangesMap = Map<string, Map<string, CellChange>>;

// useResultsTable function types
export type IsPrimaryKeyColumnFn = (columnName: string) => boolean;
export type IsColumnEditableFn = (columnName: string) => boolean;
export type UpdateCellFn = (
  rowIndex: number,
  columnId: string,
  originalValue: string | null,
  newValue: string,
) => void;
export type IsRowModifiedFn = (rowIndex: number) => boolean;
export type IsCellModifiedFn = (rowIndex: number, columnId: string) => boolean;
export type GetCellDisplayValueFn = (
  rowIndex: number,
  columnId: string,
  originalValue: string | null,
) => string | null;
export type ModifiedRow = { pkKey: string; changes: Map<string, CellChange> };
export type GetModifiedRowsFn = () => ModifiedRow[];
export type DiscardChangesFn = () => void;

export interface UseResultsTableParams {
  rows: DataTableRow[];
  columns: QueryColumnInfo[];
  editableInfo: EditableInfo | null | undefined;
}

export interface UseResultsTableReturn {
  isEditable: boolean;
  isPrimaryKeyColumn: IsPrimaryKeyColumnFn;
  isColumnEditable: IsColumnEditableFn;
  updateCell: UpdateCellFn;
  isRowModified: IsRowModifiedFn;
  isCellModified: IsCellModifiedFn;
  getCellDisplayValue: GetCellDisplayValueFn;
  getModifiedRows: GetModifiedRowsFn;
  discardChanges: DiscardChangesFn;
  changesCount: number;
}
