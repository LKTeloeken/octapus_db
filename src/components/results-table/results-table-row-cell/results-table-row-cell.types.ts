import type { QueryColumnInfo } from '@/api/database/database-responses.types';
import type {
  DataTableRow,
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
  isEven: boolean;
  rowHeight: number;
  rowStart: number;

  // Row cell properties
  columns: QueryColumnInfo[];
  getCellDisplayValue: GetCellDisplayValueFn;
  isCellModified: IsCellModifiedFn;
  isColumnEditable: IsColumnEditableFn;
  updateCell: UpdateCellFn;
}
