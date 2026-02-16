export interface DataTableCellProps {
  value: string | null;
  displayValue: string | null;
  isEditable: boolean;
  isModified: boolean;
  onSave: (newValue: string) => void;
}
