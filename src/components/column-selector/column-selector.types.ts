import type { QueryColumnInfo } from '@/api/types/query.types';

export interface ColumnSelectorProps {
  columns: QueryColumnInfo[];
  /** Nomes das colunas ocultas (vazio = todas visíveis) */
  hiddenColumns: Set<string>;
  /** Recebe a nova lista de colunas ocultas */
  onChange: (hiddenColumns: string[]) => void;
}
