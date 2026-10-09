import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { getSettings, updateSettings } from '@/api/settings';
import type { AppSettings } from '@/api/types/settings.types';
import { queryKeys } from './keys';

export const settingsQuery = {
  queryKey: queryKeys.settings,
  queryFn: getSettings,
  staleTime: Infinity, // SQLite local — só muda pela mutação abaixo
} as const;

export function useSettings() {
  return useQuery(settingsQuery);
}

/**
 * Grava um campo (ou mais) das preferências. O cache muda na hora, para o
 * controle responder sem esperar o disco, e volta atrás se a gravação falhar.
 */
export function useUpdateSettings() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (patch: Partial<AppSettings>) => {
      const current =
        queryClient.getQueryData<AppSettings>(queryKeys.settings) ??
        (await queryClient.fetchQuery(settingsQuery));
      return updateSettings({ ...current, ...patch } as AppSettings);
    },
    onMutate: async patch => {
      await queryClient.cancelQueries({ queryKey: queryKeys.settings });
      const previous = queryClient.getQueryData<AppSettings>(
        queryKeys.settings,
      );
      if (previous) {
        queryClient.setQueryData(queryKeys.settings, { ...previous, ...patch });
      }
      return { previous };
    },
    onError: (_error, _patch, context) => {
      if (context?.previous) {
        queryClient.setQueryData(queryKeys.settings, context.previous);
      }
    },
    onSuccess: saved => {
      queryClient.setQueryData(queryKeys.settings, saved);
    },
  });
}
