import { useQuery, useQueryClient } from '@tanstack/react-query';
import { getAssistantMe } from '@/api/assistant';
import type { AssistantMe } from '@/api/types/assistant.types';
import { assistantQueryFn, retryAssistant } from './assistant-errors';
import { queryKeys } from './keys';

/**
 * O termo precisa ser mostrado (RN-11): há um vigente e o aceito não é ele.
 * Sem termo publicado (`currentVersion` nulo) não há o que aceitar — o
 * assistente fica indisponível até a API publicar um.
 */
export function consentPending(me: AssistantMe): boolean {
  const { currentVersion, acceptedVersion } = me.consent;
  return currentVersion !== null && acceptedVersion !== currentVersion;
}

/** Tokens que o participante ainda pode gastar (nunca negativo) */
export function quotaRemaining(me: AssistantMe): number {
  const { limitTokens, usedTokens, reservedTokens } = me.quota;
  return Math.max(0, limitTokens - usedTokens - reservedTokens);
}

/** Participante, consentimento, cota e modo de seleção; só com login */
export function useAssistantMe(enabled = true) {
  const queryClient = useQueryClient();

  return useQuery({
    queryKey: queryKeys.assistantMe,
    queryFn: assistantQueryFn(queryClient, getAssistantMe),
    enabled,
    retry: retryAssistant,
  });
}
