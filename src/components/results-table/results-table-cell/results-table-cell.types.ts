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
  onSave: (newValue: string) => void;
}
