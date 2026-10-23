import type { QueryColumnInfo } from '@/api/types/query.types';
import type { GridMatcher } from '@/lib/grid-search';
import type {
  ActivateCellFn,
  ActiveCell,
  CellPosition,
  DataTableRow,
  FocusCellFn,
  FocusedCell,
  GetCellDisplayValueFn,
  IsCellModifiedFn,
  IsColumnEditableFn,
  IsPrimaryKeyColumnFn,
  IsRowFlagFn,
  IsRowModifiedFn,
  UpdateCellFn,
} from '../results-table.types';

export interface ResultsTableVerticalProps {
  /** Colunas a renderizar (já sem as ocultas) */
  columns: QueryColumnInfo[];
  /** Índice original em cada `row` de cada coluna de `columns` */
  columnIndices: number[];
  rows: DataTableRow[];
  isPrimaryKeyColumn: IsPrimaryKeyColumnFn;
  isColumnEditable: IsColumnEditableFn;
  isCellModified: IsCellModifiedFn;
  isRowModified: IsRowModifiedFn;
  isRowAdded: IsRowFlagFn;
  isRowRemoved: IsRowFlagFn;
  isRowSelected: IsRowFlagFn;
  isColumnSelected: (columnName: string) => boolean;
  getCellDisplayValue: GetCellDisplayValueFn;
  updateCell: UpdateCellFn;
  activeCell: ActiveCell;
  /** Célula do cursor — aqui só marca qual célula o painel de valor mostra */
  focusedCell?: FocusedCell;
  /** Matcher da busca (Cmd/Ctrl+F) por coluna; null fora do escopo/sem busca */
  getColumnMatcher: (columnName: string) => GridMatcher | null;
  /** Resultado atual da busca — a visão rola até ele */
  currentMatch: CellPosition | null;
  onActivateCell: ActivateCellFn;
  onCloseCell: () => void;
  /** Clique numa célula → move o cursor para ela */
  onFocusCell?: FocusCellFn;
  onSelectRow: (rowIndex: number, event: React.MouseEvent) => void;
  onSelectColumn: (columnName: string) => void;
  hasMore?: boolean;
  isLoadingMore?: boolean;
  onLoadMore?: () => void;
  /** Teclas com o foco na vista (o Tab que volta para a tabela) */
  onKeyDown?: (event: React.KeyboardEvent) => void;
}
