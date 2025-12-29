import type { DatabaseStructure } from "@/shared/models/database.types";
import type { RefObject } from "react";

export interface SQLEditorProps {
  value: string;
  onChange: (value: string) => void;
  onChangeSelection?: OnSelectionChange;
  onRunQuery?: () => void;
  className?: string;
  customRef?: RefObject<any>;

  databaseStructure: DatabaseStructure | null;
}

type OnSelectionChange = (selection: { start: number; end: number }) => void;
