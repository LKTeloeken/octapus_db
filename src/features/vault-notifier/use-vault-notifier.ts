import { useVaultStatus } from '@/queries/use-vault-status';

export function useVaultNotifier() {
  const { data } = useVaultStatus();

  return {
    /** O `vault.key` da máquina não abre mais o que está guardado no `app.db` */
    isBroken: data?.healthy === false,
  };
}
