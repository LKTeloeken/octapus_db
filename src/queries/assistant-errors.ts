import type { QueryClient } from '@tanstack/react-query';
import { hasErrorCode } from '@/api/client';
import { queryKeys } from './keys';

/**
 * Só a API fora do ar merece uma segunda tentativa: login recusado, termo
 * pendente ou cota esgotada não mudam repetindo o pedido.
 */
export function retryAssistant(failureCount: number, error: unknown) {
  return failureCount < 1 && hasErrorCode(error, 'ASSISTANT_UNAVAILABLE');
}

/**
 * Um `ASSISTANT_UNAUTHORIZED` quer dizer que o backend já apagou o token (ou
 * nunca houve): o status volta a "sem login" e os dados da sessão saem do cache.
 */
export function handleAssistantError(queryClient: QueryClient, error: unknown) {
  if (!hasErrorCode(error, 'ASSISTANT_UNAUTHORIZED')) return;
  queryClient.removeQueries({ queryKey: queryKeys.assistantSession });
  void queryClient.invalidateQueries({ queryKey: queryKeys.assistantStatus });
}

/** `queryFn` que avisa o cache do login perdido antes de rejeitar */
export function assistantQueryFn<T>(
  queryClient: QueryClient,
  fetch: () => Promise<T>,
) {
  return async () => {
    try {
      return await fetch();
    } catch (error) {
      handleAssistantError(queryClient, error);
      throw error;
    }
  };
}
