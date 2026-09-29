import type { EditableInfo } from '@/api/types/query.types';
import type { ResultsViewMode } from '../results-table.types';

export interface DataTableStatusBarProps {
  executionTimeMs?: number;
  rowCount?: number;
  rowsLength: number;
  totalCount?: number | null;
  isEditable: boolean;
  editableInfo?: EditableInfo | null;
  isLoadingMore: boolean;
  hasMore: boolean;
  viewMode: ResultsViewMode;
  onViewModeChange: (mode: ResultsViewMode) => void;
  onAddRow: () => void;
  onExport: () => void;
}
