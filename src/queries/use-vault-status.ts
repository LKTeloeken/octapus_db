import { useQuery } from '@tanstack/react-query';
import { getVaultStatus } from '@/api/vault';
import { queryKeys } from './keys';

export function useVaultStatus() {
  return useQuery({
    queryKey: queryKeys.vaultStatus,
    queryFn: getVaultStatus,
    // O backend mede o canário uma vez, no boot — não muda em runtime.
    staleTime: Infinity,
  });
}
