import type {
  EditableInfo,
  QueryColumnInfo,
  RowEdit,
} from '@/api/database/database-responses.types';
import type { ApplyQueryTabChanges } from '@/shared/hooks/use-query-tabs/use-query-tabs.types';

export type DataTableRow = (string | null)[];

export interface ResultsTableProps {
  columns: QueryColumnInfo[];
  rows: DataTableRow[];
  onLoadMore: () => void;
  onApplyChanges: (edits: RowEdit[]) => void;
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
export type GetModifiedRowsFn = () => RowEdit[];
export type DiscardChangesFn = () => void;
export type ApplyChangesFn = () => void;

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
  applyChanges: ApplyChangesFn;
  changesCount: number;
}
