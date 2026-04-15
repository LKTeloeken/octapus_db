import type { EditableInfo } from '@/api/database/database-responses.types';
import type { ApplyChangesFn } from '../results-table.types';
import type { ResultsViewLayout } from '@/stores/slices/schemaCache.types';
import type { TabType } from '@/shared/models/tabs.types';
import type { QueryColumnInfo } from '@/api/database/database-responses.types';

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
  tabType?: TabType;
  viewLayout: ResultsViewLayout;
  onViewLayoutChange?: (layout: ResultsViewLayout) => void;
  onSwitchToSql?: () => void;
  columns?: QueryColumnInfo[];
  visibleColumnNames?: string[];
  onToggleColumnVisibility?: (columnName: string) => void;
  onShowAllColumns?: () => void;
  onAddRow?: () => void;
  pendingInsertRowsCount?: number;
  pendingDeleteRowsCount?: number;
}
