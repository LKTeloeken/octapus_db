import { useQuery } from '@tanstack/react-query';
import { listDatabases } from '@/api/structure';
import { STRUCTURE_STALE_TIME_MS } from '@/providers/query-provider';
import { queryKeys } from './keys';

export function useDatabases(
  serverId: number | null | undefined,
  options?: { enabled?: boolean },
) {
  return useQuery({
    queryKey: queryKeys.databases(serverId ?? -1),
    queryFn: () => listDatabases(serverId as number),
    enabled: serverId != null && (options?.enabled ?? true),
    staleTime: STRUCTURE_STALE_TIME_MS,
    gcTime: STRUCTURE_STALE_TIME_MS,
  });
}
