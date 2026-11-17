import type { QueryColumnInfo } from '@/api/types/query.types';

export interface ColumnCellProps {
  column: QueryColumnInfo;
  isPrimaryKeyColumn: boolean;
  /** This column is the one currently ordering the data */
  isSorted?: boolean;
  /** Direction of the active sort (only when `isSorted`) */
  sortDirection?: 'asc' | 'desc';
  /** A coluna está selecionada (clique no cabeçalho) */
  isSelected?: boolean;
  className?: string;
  /**
   * Ordena por esta coluna (clique no ícone; alterna asc↔desc na ativa). Sem
   * ele não há ícone: a grade não ordena.
   */
  onSort?: (columnName: string) => void;
  /** Clique no cabeçalho: seleciona a coluna; `additive` com Shift/Cmd */
  onSelect: (columnName: string, additive: boolean) => void;
}
