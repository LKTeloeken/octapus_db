import type { DatabaseStructure } from "@/shared/models/database.types";

export interface QueryEditorContainerProps {
  value: string;
  onChange: (value: string) => void;
  onChangeSelection?: (selection: string) => void;
  onExecute: () => void;
  databaseStructure: DatabaseStructure | null;
  isLoading?: boolean;
}

export type OnSelectionChange = (selection: {
  start: number;
  end: number;
}) => void;
