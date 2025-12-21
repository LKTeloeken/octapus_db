export interface QueryEditorContainerProps {
  value: string;
  onChange: (value: string) => void;
  onChangeSelection?: (selection: string) => void;
  onExecute: () => void;
  isLoading?: boolean;
}
