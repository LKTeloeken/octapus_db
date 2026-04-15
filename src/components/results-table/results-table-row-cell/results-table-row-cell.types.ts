import type { QueryColumnInfo } from '@/api/database/database-responses.types';
import type {
  DataTableRow,
  GetCellDisplayValueFn,
  IsCellModifiedFn,
  IsColumnEditableFn,
  UpdateCellFn,
} from '../results-table.types';

export interface VisibleColumn {
  column: QueryColumnInfo;
  columnIndex: number;
}

export interface ResultsTableRowCellProps {
  // Row properties
  row: DataTableRow;
  rowIndex: number;
  isModified: boolean;
  isEven: boolean;
  isSelected: boolean;
  isDeleted: boolean;
  isInserted: boolean;
  rowHeight: number;
  rowStart: number;
  onSelect: () => void;

  // Row cell properties
  visibleColumns: VisibleColumn[];
  getCellDisplayValue: GetCellDisplayValueFn;
  isCellModified: IsCellModifiedFn;
  isColumnEditable: IsColumnEditableFn;
  updateCell: UpdateCellFn;
}
