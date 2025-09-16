import type { RefObject } from "react";

export type OnSelectionChange = (selection: {
  start: number;
  end: number;
}) => void;

export interface SQLEditorProps {
  value: string;
  onChange: (value: string) => void;
  onChangeSelection?: OnSelectionChange;
  className?: string;
  customRef?: RefObject<any>;
}
