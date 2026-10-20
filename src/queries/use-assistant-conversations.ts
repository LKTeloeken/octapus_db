import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  askAssistant,
  cancelAssistantTurn,
  createAssistantConversation,
  executeAssistantQuery,
  getAssistantConversation,
  listAssistantConversations,
  type AskAssistantInput,
  type ExecuteAssistantQueryInput,
} from '@/api/assistant';
import type { AssistantTurnEvent } from '@/api/types/assistant.types';
import {
  assistantQueryFn,
  handleAssistantError,
  retryAssistant,
} from './assistant-errors';
import { queryKeys } from './keys';

/** As conversas do participante (primeira página), do servidor (C-15) */
export function useAssistantConversations(enabled = true) {
  const queryClient = useQueryClient();

  return useQuery({
    queryKey: queryKeys.assistantConversations,
    queryFn: assistantQueryFn(queryClient, () => listAssistantConversations()),
    enabled,
    retry: retryAssistant,
  });
}

/** Uma conversa com os turnos; `null` = nenhuma aberta */
export function useAssistantConversation(conversationId: string | null) {
  const queryClient = useQueryClient();

  return useQuery({
    queryKey: queryKeys.assistantConversation(conversationId ?? ''),
    queryFn: assistantQueryFn(queryClient, () =>
      getAssistantConversation(conversationId as string),
    ),
    enabled: conversationId !== null,
    retry: retryAssistant,
  });
}

export function useCreateAssistantConversation() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({
      serverId,
      modelId = null,
      title = null,
    }: {
      serverId: number;
      modelId?: string | null;
      title?: string | null;
    }) => createAssistantConversation(serverId, modelId, title),
    onError: error => handleAssistantError(queryClient, error),
    onSuccess: () =>
      queryClient.invalidateQueries({
        queryKey: queryKeys.assistantConversations,
      }),
  });
}

/**
 * Faz a pergunta. O andamento vai para `onEvent`; ao terminar, a conversa e
 * a cota (`/me`) são relidas do servidor.
 */
export function useAskAssistant(onEvent: (event: AssistantTurnEvent) => void) {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: AskAssistantInput) => askAssistant(input, onEvent),
    onError: error => handleAssistantError(queryClient, error),
    onSettled: (_finish, _error, input) => {
      void queryClient.invalidateQueries({
        queryKey: queryKeys.assistantConversation(input.conversationId),
      });
      void queryClient.invalidateQueries({
        queryKey: queryKeys.assistantConversations,
      });
      void queryClient.invalidateQueries({ queryKey: queryKeys.assistantMe });
    },
  });
}

export function useCancelAssistantTurn() {
  return useMutation({ mutationFn: cancelAssistantTurn });
}

/** Execução manual (revisão e páginas seguintes), sempre só-leitura */
export function useExecuteAssistantQuery() {
  return useMutation({
    mutationFn: (input: ExecuteAssistantQueryInput) =>
      executeAssistantQuery(input),
  });
}
