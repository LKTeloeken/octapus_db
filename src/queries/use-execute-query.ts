import { useMutation } from '@tanstack/react-query';
import { executeQuery } from '@/api/query';
import type { QueryMessage, QueryOptions } from '@/api/types/query.types';

export const QUERY_PAGE_SIZE = 500;

export interface ExecuteQueryVariables {
  serverId: number;
  database: string;
  query: string;
  options?: QueryOptions;
  /** Recebe as mensagens do banco em streaming; omitir desliga o canal */
  onMessage?: (message: QueryMessage) => void;
}

/**
 * Free-editor execution (native syntax per database — BACKEND.md §5).
 * Results are intentionally NOT cached: each run is a fresh execution.
 * Pagination of editor results lives in the feature hook, which re-runs
 * with a higher offset and appends rows.
 */
export function useExecuteQuery() {
  return useMutation({
    mutationFn: ({
      serverId,
      database,
      query,
      options,
      onMessage,
    }: ExecuteQueryVariables) =>
      executeQuery(
        serverId,
        database,
        query,
        {
          limit: QUERY_PAGE_SIZE,
          offset: 0,
          countTotal: true,
          ...options,
        },
        onMessage,
      ),
  });
}
