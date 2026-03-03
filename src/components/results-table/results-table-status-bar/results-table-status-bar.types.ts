import type { EditableInfo } from '@/api/database/database-responses.types';
import type { ApplyChangesFn } from '../results-table.types';

export interface DataTableStatusBarProps {
  executionTimeMs?: number;
  rowCount?: number;
  rowsLength: number;
  totalCount?: number | null;
  isEditable: boolean;
  editableInfo?: EditableInfo | null;
  changesCount: number;
  isLoadingMore: boolean;
  hasMore: boolean;
  onDiscardChanges: () => void;
  onApplyChanges: ApplyChangesFn;
}
