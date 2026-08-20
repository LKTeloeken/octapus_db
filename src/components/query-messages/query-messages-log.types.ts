import type { QueryLogEntry } from '@/features/query-editor/query-results-store';

export type { QueryLogEntry };

export interface QueryMessagesLogProps {
  className?: string;
  entries: QueryLogEntry[];
  onClear: () => void;
}
