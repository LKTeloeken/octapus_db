import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { getCliStatus, installCli } from '@/api/cli';
import { queryKeys } from './keys';

/** O `octapus` de terminal nesta máquina. Só muda quando o app o instala. */
export function useCliStatus() {
  return useQuery({
    queryKey: queryKeys.cliStatus,
    queryFn: getCliStatus,
    staleTime: Infinity,
  });
}

/** Instala (ou atualiza) o `octapus`; a resposta já é o estado novo. */
export function useInstallCli() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: installCli,
    onSuccess: status => queryClient.setQueryData(queryKeys.cliStatus, status),
  });
}
