import { QueryResultsRow } from "@/shared/models/database.types";

export interface ResultsContainerProps {
  columns: string[];
  rows: QueryResultsRow[];
  isLoading?: boolean;
  error?: string | null;
  className?: string;
}
