import type { QueryLogEntry } from '@/stores/query-results-store';

export type { QueryLogEntry };

export interface QueryMessagesLogProps {
  className?: string;
  entries: QueryLogEntry[];
  onClear: () => void;
}
