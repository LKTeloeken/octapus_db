import type { QueryColumnInfo } from '@/api/database/database-responses.types';
import type { QueryResultsRow } from '@/shared/models/database.types';

export interface ResultsContainerProps {
  columns: QueryColumnInfo[];
  rows: QueryResultsRow;
  isLoading?: boolean;
  isLoadingMore?: boolean;
  hasMore?: boolean;
  executionTimeMs?: number;
  totalCount?: number | null;
  rowCount?: number;
  onLoadMore?: () => void;
  error?: string | null;
  className?: string;
}
