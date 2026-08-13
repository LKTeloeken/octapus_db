import type { VirtualItem } from '@tanstack/react-virtual';
import type { QueryColumnInfo } from '@/api/types/query.types';
import type {
  ActivateCellFn,
  DataTableRow,
  FocusCellFn,
  GetCellDisplayValueFn,
  IsCellModifiedFn,
  IsColumnEditableFn,
  UpdateCellFn,
} from '../results-table.types';

export interface ResultsTableRowCellProps {
  // Row properties
  row: DataTableRow;
  rowIndex: number;
  isModified: boolean;
  /** Pending insert (green) */
  isAdded: boolean;
  /** Pending delete, still visible (red) */
  isRemoved: boolean;
  isSelected: boolean;
  isEven: boolean;
  rowHeight: number;
  rowStart: number;
  /** Sticky left identifier/selection column width */
  gutterWidth: number;
  /** Full virtualized row width (gutter + all columns) */
  totalWidth: number;

  // Row cell properties
  /** Colunas a renderizar (já sem as ocultas) */
  columns: QueryColumnInfo[];
  /** Índice original em `row` de cada coluna de `columns` */
  columnIndices: number[];
  /** Only the horizontally-visible columns are rendered */
  virtualColumns: VirtualItem[];
  getCellDisplayValue: GetCellDisplayValueFn;
  isCellModified: IsCellModifiedFn;
  isColumnEditable: IsColumnEditableFn;
  updateCell: UpdateCellFn;
  /** Column name of the active cell in THIS row, or null. Kept row-scoped so
   *  activating a cell only re-renders the affected row(s). */
  activeColumnName: string | null;
  /** Column name do cursor de teclado NESTA linha, ou null — row-scoped pelo
   *  mesmo motivo do `activeColumnName`. */
  focusedColumnName: string | null;
  onActivateCell: ActivateCellFn;
  onCloseCell: () => void;
  /** Clique numa célula → move o cursor de teclado para ela */
  onFocusCell: FocusCellFn;
  /** Clique no corpo da linha COM modificador → estende/alterna a seleção */
  onSelectRowBody: (rowIndex: number, event: React.MouseEvent) => void;
  /** Click on the index gutter → add to the selection (Shift = range) */
  onSelectRowGutter: (rowIndex: number, event: React.MouseEvent) => void;
}
