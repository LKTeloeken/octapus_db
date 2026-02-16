import type {
  EditableInfo,
  QueryColumnInfo,
} from '@/api/database/database-responses.types';

export type DataTableRow = (string | null)[];

export interface DataTableProps {
  columns: QueryColumnInfo[];
  rows: DataTableRow[];
  editableInfo?: EditableInfo | null;
  isLoading?: boolean;
  isLoadingMore?: boolean;
  hasMore?: boolean;
  executionTimeMs?: number;
  totalCount?: number | null;
  rowCount?: number;
  onLoadMore?: () => void;
  className?: string;
}

export interface CellChange {
  columnId: string;
  originalValue: string | null;
  newValue: string;
}

export type ChangesMap = Map<string, Map<string, CellChange>>;
