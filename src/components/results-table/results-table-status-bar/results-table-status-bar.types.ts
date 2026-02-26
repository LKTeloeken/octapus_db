import type { EditableInfo } from '@/api/database/database-responses.types';

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
}
