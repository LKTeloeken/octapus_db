import type { QueryColumnInfo } from '@/api/types/query.types';

export interface ColumnCellProps {
  column: QueryColumnInfo;
  isPrimaryKeyColumn: boolean;
  /** This column is the one currently ordering the data */
  isSorted?: boolean;
  /** Direction of the active sort (only when `isSorted`) */
  sortDirection?: 'asc' | 'desc';
  className?: string;
  /** Sort by this column (toggles asc↔desc on the active one) */
  onSort: (columnName: string) => void;
}
