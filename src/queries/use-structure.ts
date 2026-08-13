import { useQuery, useQueryClient } from '@tanstack/react-query';
import { listSchemasWithTables } from '@/api/structure';
import { STRUCTURE_STALE_TIME_MS } from '@/providers/query-provider';
import { queryKeys } from './keys';
import { useCallback } from 'react';

/** Full schemas+tables tree of one database (sidebar + editor autocomplete) */
export function useStructure(
  serverId: number | null | undefined,
  database: string | null | undefined,
  options?: { enabled?: boolean },
) {
  return useQuery({
    queryKey: queryKeys.structure(serverId ?? -1, database ?? ''),
    queryFn: () => listSchemasWithTables(serverId as number, database as string),
    enabled: serverId != null && !!database && (options?.enabled ?? true),
    staleTime: STRUCTURE_STALE_TIME_MS,
    gcTime: STRUCTURE_STALE_TIME_MS,
  });
}

/** Force a fresh structure fetch (manual refresh / after DDL) */
export function useInvalidateStructure() {
  const queryClient = useQueryClient();

  return useCallback(
    (serverId: number, database: string) =>
      queryClient.invalidateQueries({
        queryKey: queryKeys.structure(serverId, database),
      }),
    [queryClient],
  );
}
