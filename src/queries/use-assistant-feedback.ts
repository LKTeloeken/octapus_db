import {
  useMutation,
  useQueryClient,
  type QueryClient,
} from '@tanstack/react-query';
import { clearAssistantFeedback, setAssistantFeedback } from '@/api/assistant';
import type {
  AssistantConversation,
  AssistantFeedback,
  AssistantFeedbackInput,
} from '@/api/types/assistant.types';
import {
  optimisticFeedback,
  turnFeedbackOf,
  withTurnFeedback,
} from '@/lib/assistant/feedback';
import { handleAssistantError } from './assistant-errors';
import { queryKeys } from './keys';

/** A avaliação nova do turno; `null` remove */
export type FeedbackChange = AssistantFeedbackInput | null;

interface FeedbackContext {
  previous: AssistantFeedback | null;
}

/** Sob o prefixo do assistente: mutação pausada não vai para o disco (RN-01) */
export const feedbackMutationKey = (turnId: string) =>
  ['assistant', 'feedback', turnId] as const;

/**
 * Avaliar o turno com atualização otimista: a conversa no cache já mostra a
 * avaliação nova e volta à anterior se o servidor recusar. As mudanças do
 * mesmo turno saem em fila (`scope`), na ordem dos cliques; a conversa só é
 * relida quando a fila esvazia, para não piscar o estado velho no meio.
 */
export function feedbackMutationOptions(
  queryClient: QueryClient,
  conversationId: string,
  turnId: string,
) {
  const conversationKey = queryKeys.assistantConversation(conversationId);
  const mutationKey = feedbackMutationKey(turnId);

  const write = (feedback: AssistantFeedback | null) =>
    queryClient.setQueryData<AssistantConversation>(
      conversationKey,
      conversation =>
        conversation && withTurnFeedback(conversation, turnId, feedback),
    );

  return {
    mutationKey,
    scope: { id: mutationKey.join(':') },
    // Sem rede o backend responde ASSISTANT_UNAVAILABLE (e a tela desfaz);
    // nada fica pausado esperando a conexão voltar
    networkMode: 'always' as const,
    mutationFn: (change: FeedbackChange) =>
      change
        ? setAssistantFeedback(turnId, change)
        : clearAssistantFeedback(turnId),
    onMutate: async (change: FeedbackChange): Promise<FeedbackContext> => {
      // Uma releitura no meio do caminho traria o estado velho por cima
      await queryClient.cancelQueries({ queryKey: conversationKey });
      const previous = turnFeedbackOf(
        queryClient.getQueryData<AssistantConversation>(conversationKey),
        turnId,
      );
      write(
        change
          ? optimisticFeedback(change, previous, new Date().toISOString())
          : null,
      );
      return { previous };
    },
    onError: (
      error: unknown,
      _change: FeedbackChange,
      context: FeedbackContext | undefined,
    ) => {
      if (context) write(context.previous);
      handleAssistantError(queryClient, error);
    },
    onSettled: () => {
      // Esta mudança ainda conta como pendente aqui: 1 = a fila acabou
      if (queryClient.isMutating({ mutationKey }) > 1) return;
      void queryClient.invalidateQueries({ queryKey: conversationKey });
    },
  };
}

/** A avaliação de um turno da conversa (👍/👎, motivos e comentário) */
export function useTurnFeedbackMutation(
  conversationId: string,
  turnId: string,
) {
  const queryClient = useQueryClient();
  return useMutation(
    feedbackMutationOptions(queryClient, conversationId, turnId),
  );
}
