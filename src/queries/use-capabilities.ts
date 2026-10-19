import { useQuery } from '@tanstack/react-query';
import { useMemo } from 'react';
import { getCapabilities, getDbCapabilities } from '@/api/connection';
import type {
  AdapterCapabilities,
  DbTypeCapabilities,
} from '@/api/types/capabilities.types';
import type { DatabaseType } from '@/api/types/server.types';
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

/** Capabilities por tipo de banco; tipo ausente = ainda carregando */
export type CapabilitiesByType = Partial<
  Record<DatabaseType, AdapterCapabilities>
>;

export const toCapabilitiesByType = (
  list: DbTypeCapabilities[] | undefined,
): CapabilitiesByType =>
  Object.fromEntries(
    (list ?? []).map(entry => [entry.dbType, entry.capabilities]),
  );

/**
 * A tabela de capabilities por tipo — o que a árvore e a palette consultam
 * para decidir por servidor (`caps[server.dbType]`) sem uma consulta por
 * servidor. Estática: busca uma vez por execução.
 */
export function useDbCapabilities(): CapabilitiesByType {
  const { data } = useQuery({
    queryKey: queryKeys.dbCapabilities,
    queryFn: getDbCapabilities,
    staleTime: Infinity,
    gcTime: Infinity,
  });
  return useMemo(() => toCapabilitiesByType(data), [data]);
}
