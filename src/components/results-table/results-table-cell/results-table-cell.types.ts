export type CellEditorType =
  | 'text'
  | 'number'
  | 'boolean'
  | 'json'
  | 'date'
  | 'datetime'
  | 'time'
  | 'uuid'
  | 'enum'
  | 'readonly';

export interface DataTableCellProps {
  value: string | null;
  displayValue: string | null;
  isEditable: boolean;
  isModified: boolean;
  columnType: string;
  /** Position — used to build the save closure and the activate call */
  rowIndex: number;
  columnName: string;
  /** Whether this is the single cell currently open for editing/viewing */
  isActive: boolean;
  /** Open this cell's editor/viewer (stable) */
  onActivate: (rowIndex: number, columnName: string) => void;
  /** Close the active cell (stable) */
  onClose: () => void;
  /** Persist a cell change (stable); newValue null grava NULL */
  updateCell: (
    rowIndex: number,
    columnId: string,
    originalValue: string | null,
    newValue: string | null,
  ) => void;
}
