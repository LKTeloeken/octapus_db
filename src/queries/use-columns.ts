import { useQuery } from '@tanstack/react-query';
import { listColumns } from '@/api/structure';
import { STRUCTURE_STALE_TIME_MS } from '@/providers/query-provider';
import { queryKeys } from './keys';

export interface UseColumnsParams {
  serverId: number;
  database: string;
  schema: string;
  table: string;
}

export function useColumns(
  params: UseColumnsParams | null,
  options?: { enabled?: boolean },
) {
  return useQuery({
    queryKey: queryKeys.columns(
      params?.serverId ?? -1,
      params?.database ?? '',
      params?.schema ?? '',
      params?.table ?? '',
    ),
    queryFn: () =>
      listColumns(
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
