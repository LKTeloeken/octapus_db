export interface QueryEditorToolbarProps {
  onRun: () => void;
  onFormat: () => void;
  onCancel?: () => void;
  isLoading?: boolean;
  isQueryRunning?: boolean;
  disabled?: boolean;
}
