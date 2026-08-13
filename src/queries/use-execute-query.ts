import { useMutation } from '@tanstack/react-query';
import { executeQuery } from '@/api/query';
import type { QueryOptions } from '@/api/types/query.types';

export const QUERY_PAGE_SIZE = 500;

export interface ExecuteQueryVariables {
  serverId: number;
  database: string;
  query: string;
  options?: QueryOptions;
}

/**
 * Free-editor execution (native syntax per database — BACKEND.md §5).
 * Results are intentionally NOT cached: each run is a fresh execution.
 * Pagination of editor results lives in the feature hook, which re-runs
 * with a higher offset and appends rows.
 */
export function useExecuteQuery() {
  return useMutation({
    mutationFn: ({ serverId, database, query, options }: ExecuteQueryVariables) =>
      executeQuery(serverId, database, query, {
        limit: QUERY_PAGE_SIZE,
        offset: 0,
        countTotal: true,
        ...options,
      }),
  });
}
