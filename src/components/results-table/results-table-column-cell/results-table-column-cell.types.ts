import type { QueryColumnInfo } from '@/api/database/database-responses.types';

export interface ColumnCellProps {
  column: QueryColumnInfo;
  isPrimaryKeyColumn: boolean;
  className?: string;
}
