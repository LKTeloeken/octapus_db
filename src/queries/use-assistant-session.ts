import { useMutation, useQueryClient } from '@tanstack/react-query';
import { logoutAssistant, redeemAssistantInvite } from '@/api/assistant';
import { queryKeys } from './keys';

/** Entra com o código de convite; o token fica cifrado no backend */
export function useRedeemAssistantInvite() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({
      inviteCode,
      deviceLabel = null,
    }: {
      inviteCode: string;
      deviceLabel?: string | null;
    }) => redeemAssistantInvite(inviteCode, deviceLabel),
    onSuccess: () => {
      // Outro participante pode ter entrado: nada da sessão anterior serve
      queryClient.removeQueries({ queryKey: queryKeys.assistantSession });
      return queryClient.invalidateQueries({
        queryKey: queryKeys.assistantStatus,
      });
    },
  });
}

/** Sai: esquece o token nesta máquina e limpa o cache do assistente */
export function useLogoutAssistant() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: logoutAssistant,
    onSuccess: () => {
      queryClient.removeQueries({ queryKey: queryKeys.assistantSession });
      return queryClient.invalidateQueries({
        queryKey: queryKeys.assistantStatus,
      });
    },
  });
}
