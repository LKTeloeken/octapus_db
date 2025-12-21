import type { QueryResultsRow } from "@/shared/models/database.types";

export interface ResultsTableProps {
  columns: Array<string>;
  rows: QueryResultsRow[];
  rowHeight?: number;
  overscan?: number;
  className?: string;
  emptyMessage?: string;
}
