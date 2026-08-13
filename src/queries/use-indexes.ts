import { useQuery } from '@tanstack/react-query';
import { listIndexes } from '@/api/structure';
import { STRUCTURE_STALE_TIME_MS } from '@/providers/query-provider';
import { queryKeys } from './keys';
import type { UseColumnsParams } from './use-columns';

export function useIndexes(
  params: UseColumnsParams | null,
  options?: { enabled?: boolean },
) {
  return useQuery({
    queryKey: queryKeys.indexes(
      params?.serverId ?? -1,
      params?.database ?? '',
      params?.schema ?? '',
      params?.table ?? '',
    ),
    queryFn: () =>
      listIndexes(
        params!.serverId,
        params!.database,
        params!.schema,
        params!.table,
      ),
    enabled: params != null && (options?.enabled ?? true),
    staleTime: STRUCTURE_STALE_TIME_MS,
    gcTime: STRUCTURE_STALE_TIME_MS,
  });
}
