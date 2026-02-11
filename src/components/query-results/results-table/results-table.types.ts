import type { QueryColumnInfo } from '@/api/database/database-responses.types';
import type { QueryResultsRow } from '@/shared/models/database.types';

export interface ResultsTableProps {
  columns: QueryColumnInfo[];
  rows: QueryResultsRow;
  rowHeight?: number;
  overscan?: number;
  className?: string;
  emptyMessage?: string;
}
