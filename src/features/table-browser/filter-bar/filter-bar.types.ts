import type { ColumnFilter } from '@/api/types/browse.types';
import type { QueryColumnInfo } from '@/api/types/query.types';

export interface FilterBarProps {
  columns: QueryColumnInfo[];
  filters: ColumnFilter[];
  onChange: (filters: ColumnFilter[]) => void;
}
