import type { RefObject } from "react";

export interface SQLEditorProps {
  value: string;
  onChange: (value: string) => void;
  onChangeSelection?: OnSelectionChange;
  onRunQuery?: () => void;
  className?: string;
  customRef?: RefObject<any>;
}

type OnSelectionChange = (selection: { start: number; end: number }) => void;
