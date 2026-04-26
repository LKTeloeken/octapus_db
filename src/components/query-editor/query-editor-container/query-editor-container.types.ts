import type { DatabaseStructure } from '@/shared/models/database.types';

export interface QueryEditorContainerProps {
  value: string;
  onChange: (value: string) => void;
  onExecute: (query: string) => void;
  databaseStructure: DatabaseStructure | null;
}
