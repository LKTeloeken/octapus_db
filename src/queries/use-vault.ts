import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  disableMasterPassword,
  enableMasterPassword,
  getVaultStatus,
  lockVault,
  resetVault,
  unlockVault,
} from '@/api/vault';
import { useConnectionStore } from '@/stores/connection-store';
import { queryKeys } from './keys';

export function useVaultStatus() {
  return useQuery({
    queryKey: queryKeys.vaultStatus,
    queryFn: getVaultStatus,
  });
}

/** Toda operação de cofre muda o status; nenhuma delas pode deixá-lo obsoleto */
function useVaultMutation<TArg>(mutationFn: (arg: TArg) => Promise<void>) {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.vaultStatus });
    },
  });
}

export function useUnlockVault() {
  const queryClient = useQueryClient();
  const mutation = useVaultMutation(unlockVault);

  return {
    ...mutation,
    unlock: async (password: string) => {
      await mutation.mutateAsync(password);
      // Com a chave em mãos, o que falhou por estar trancado pode ser refeito.
      await queryClient.invalidateQueries();
    },
  };
}

export function useLockVault() {
  const queryClient = useQueryClient();
  const mutation = useVaultMutation(() => lockVault());

  return {
    ...mutation,
    lock: async () => {
      await mutation.mutateAsync(undefined as never);
      // O backend derrubou os pools; o front não pode seguir achando que há
      // conexão aberta.
      useConnectionStore.getState().clearAll();
      await queryClient.invalidateQueries();
    },
  };
}

export function useEnableMasterPassword() {
  return useVaultMutation(enableMasterPassword);
}

export function useDisableMasterPassword() {
  return useVaultMutation(disableMasterPassword);
}

export function useResetVault() {
  const queryClient = useQueryClient();
  const mutation = useVaultMutation(() => resetVault());

  return {
    ...mutation,
    reset: async () => {
      await mutation.mutateAsync(undefined as never);
      await queryClient.invalidateQueries();
    },
  };
}
