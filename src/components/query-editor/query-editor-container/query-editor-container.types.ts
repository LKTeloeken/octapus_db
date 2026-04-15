import type { DatabaseStructure } from '@/shared/models/database.types';

export interface QueryEditorContainerProps {
  value: string;
  onChange: (value: string) => void;
  onChangeSelection?: (selection: string) => void;
  onExecute: (query: string) => void;
  onCancel?: () => void;
  databaseStructure: DatabaseStructure | null;
  onRequestTableColumns?: (schemaName: string, tableName: string) => Promise<void>;
  isLoading?: boolean;
  isQueryRunning?: boolean;
}

export type OnSelectionChange = (selection: {
  start: number;
  end: number;
}) => void;
