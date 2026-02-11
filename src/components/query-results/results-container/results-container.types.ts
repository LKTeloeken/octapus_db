import type { QueryColumnInfo } from '@/api/database/database-responses.types';
import type { QueryResultsRow } from '@/shared/models/database.types';

export interface ResultsContainerProps {
  columns: QueryColumnInfo[];
  rows: QueryResultsRow;
  isLoading?: boolean;
  error?: string | null;
  className?: string;
}
