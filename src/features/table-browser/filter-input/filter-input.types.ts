export interface FilterInputProps {
  value: string;
  onChange: (value: string) => void;
  onApply: () => void;
  onReset: () => void;
}
