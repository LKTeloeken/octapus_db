import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { acceptAssistantConsent, getAssistantConsent } from '@/api/assistant';
import { hasErrorCode } from '@/api/client';
import {
  assistantQueryFn,
  handleAssistantError,
  retryAssistant,
} from './assistant-errors';
import { queryKeys } from './keys';

/** Texto do termo vigente, para exibir antes do aceite */
export function useAssistantConsent(enabled = true) {
  const queryClient = useQueryClient();

  return useQuery({
    queryKey: queryKeys.assistantConsent,
    queryFn: assistantQueryFn(queryClient, getAssistantConsent),
    enabled,
    retry: retryAssistant,
    staleTime: Infinity, // muda só quando a API publica outro termo
  });
}

/**
 * Aceita o termo lido. `ASSISTANT_CONFLICT` = o termo mudou enquanto a pessoa
 * lia: o texto é recarregado para ela ler o novo.
 */
export function useAcceptAssistantConsent() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: acceptAssistantConsent,
    onError: error => {
      handleAssistantError(queryClient, error);
      if (hasErrorCode(error, 'ASSISTANT_CONFLICT')) {
        void queryClient.invalidateQueries({
          queryKey: queryKeys.assistantConsent,
        });
      }
    },
    onSuccess: () =>
      queryClient.invalidateQueries({ queryKey: queryKeys.assistantMe }),
  });
}
