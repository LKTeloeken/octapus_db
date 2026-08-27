import { useVaultStatus } from '@/queries/use-vault';

export function useVaultNotifier() {
  const { data } = useVaultStatus();

  return {
    /** O `vault.key` da máquina não abre mais o que está guardado no `app.db` */
    isBroken: data?.healthy === false,
    /** Ilegível em qualquer formato: nem o reset preserva nada */
    isCorrupt: data?.corrupt === true,
  };
}
