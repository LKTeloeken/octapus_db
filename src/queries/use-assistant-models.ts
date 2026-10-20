import { useQuery, useQueryClient } from '@tanstack/react-query';
import { getAssistantModels } from '@/api/assistant';
import { assistantQueryFn, retryAssistant } from './assistant-errors';
import { queryKeys } from './keys';

/**
 * Modelos para o participante escolher. Só no modo `free` — nos outros a API
 * responde `ASSISTANT_MODEL_SELECTION_FORBIDDEN`, então ligue com
 * `me.modelSelection === 'free'`.
 */
export function useAssistantModels(enabled = true) {
  const queryClient = useQueryClient();

  return useQuery({
    queryKey: queryKeys.assistantModels,
    queryFn: assistantQueryFn(queryClient, getAssistantModels),
    enabled,
    retry: retryAssistant,
  });
}
