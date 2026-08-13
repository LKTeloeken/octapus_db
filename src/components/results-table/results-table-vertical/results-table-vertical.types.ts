import type { QueryColumnInfo } from '@/api/types/query.types';
import type {
  ActivateCellFn,
  ActiveCell,
  DataTableRow,
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
  onActivateCell: ActivateCellFn;
  onCloseCell: () => void;
  onSelectRow: (rowIndex: number, event: React.MouseEvent) => void;
  onSelectColumn: (columnName: string) => void;
  hasMore?: boolean;
  isLoadingMore?: boolean;
  onLoadMore?: () => void;
}
