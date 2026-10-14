import type { QueryColumnInfo } from '@/api/types/query.types';
import type { UseGridSearchReturn } from '../use-grid-search';

export interface ResultsTableSearchProps {
  search: UseGridSearchReturn;
  /** Colunas desenhadas — as candidatas do escopo "Somente/Exceto" */
  columns: QueryColumnInfo[];
}
