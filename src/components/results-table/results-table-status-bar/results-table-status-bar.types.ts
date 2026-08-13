import type { EditableInfo } from '@/api/types/query.types';
import type { ResultsViewMode, SaveFn } from '../results-table.types';

export interface DataTableStatusBarProps {
  executionTimeMs?: number;
  rowCount?: number;
  rowsLength: number;
  totalCount?: number | null;
  isEditable: boolean;
  editableInfo?: EditableInfo | null;
  /** Cell edits pending (excludes rows being deleted) */
  changesCount: number;
  /** New rows pending insert */
  addedCount: number;
  /** Existing rows pending delete */
  removedCount: number;
  /** Total pending operations (edits + added + removed) */
  pendingCount: number;
  isLoadingMore: boolean;
  hasMore: boolean;
  viewMode: ResultsViewMode;
  onViewModeChange: (mode: ResultsViewMode) => void;
  onAddRow: () => void;
  onDiscardChanges: () => void;
  onSave: SaveFn;
}
