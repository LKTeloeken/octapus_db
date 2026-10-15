import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { detectPgTools, getPgToolsStatus } from '@/api/backup';
import { updateSettings } from '@/api/settings';
import type { AppSettings } from '@/api/types/settings.types';
import { queryKeys } from './keys';
import { settingsQuery } from './use-settings';

/**
 * Onde estão as ferramentas do Postgres. Só muda com instalação nova ou com a
 * pasta das Preferências — o refetch é manual ("Detectar de novo") ou pela
 * invalidação ao trocar a pasta.
 */
export function usePgToolsDetection() {
  return useQuery({
    queryKey: queryKeys.pgToolsDetection,
    queryFn: detectPgTools,
    staleTime: Infinity,
  });
}

/** As ferramentas frente a um servidor (abre a conexão, se preciso) */
export function usePgToolsStatus(
  serverId: number,
  database: string,
  enabled = true,
) {
  return useQuery({
    queryKey: queryKeys.pgToolsStatus(serverId, database),
    queryFn: () => getPgToolsStatus(serverId, database),
    enabled,
    staleTime: 60_000,
  });
}

/**
 * Grava a pasta das ferramentas (`null` = detectar sozinho) e refaz a busca —
 * a detecção e todo status já calculado dependem dela.
 */
export function useSetPgToolsDir() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (pgToolsDir: string | null) => {
      const current =
        queryClient.getQueryData<AppSettings>(queryKeys.settings) ??
        (await queryClient.fetchQuery(settingsQuery));
      return updateSettings({ ...current, pgToolsDir } as AppSettings);
    },
    onSuccess: saved => {
      queryClient.setQueryData(queryKeys.settings, saved);
      return queryClient.invalidateQueries({ queryKey: queryKeys.pgTools });
    },
  });
}
