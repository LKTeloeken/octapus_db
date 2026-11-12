import { useQuery } from '@tanstack/react-query';
import { getAssistantStatus } from '@/api/assistant';
import { queryKeys } from './keys';

/** O health check vai à API: um minuto de folga entre consultas */
const STATUS_STALE_TIME_MS = 60 * 1000;

/**
 * Configurado, online, logado e se o app precisa de atualização. Nunca falha
 * por causa da API: a tela decide pelos campos (C-19).
 */
export function useAssistantStatus() {
  return useQuery({
    queryKey: queryKeys.assistantStatus,
    queryFn: getAssistantStatus,
    staleTime: STATUS_STALE_TIME_MS,
  });
}

/**
 * O assistente existe neste app? Só com a URL da API no build **e** o canal
 * beta ligado (o backend responde `configured: false` fora disso): é o que
 * decide se botões, atalhos e configurações do assistente aparecem.
 */
export function useAssistantAvailable() {
  return useAssistantStatus().data?.configured === true;
}
