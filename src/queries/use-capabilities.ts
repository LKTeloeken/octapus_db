import { useQuery } from '@tanstack/react-query';
import { getCapabilities } from '@/api/connection';
import { STRUCTURE_STALE_TIME_MS } from '@/providers/query-provider';
import { queryKeys } from './keys';

/** Per-database-type UI switches (hasSchemas, supportsSql, ...) — BACKEND.md §3 */
export function useCapabilities(serverId: number | null | undefined) {
  return useQuery({
    queryKey: queryKeys.capabilities(serverId ?? -1),
    queryFn: () => getCapabilities(serverId as number),
    enabled: serverId != null,
    staleTime: STRUCTURE_STALE_TIME_MS,
    gcTime: STRUCTURE_STALE_TIME_MS,
  });
}
