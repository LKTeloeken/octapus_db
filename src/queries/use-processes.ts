import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { listProcesses, terminateProcess } from '@/api/processes';
import { queryKeys } from './keys';

const REFRESH_INTERVAL_MS = 5_000;

export function useProcesses(serverId: number) {
  return useQuery({
    queryKey: queryKeys.processes(serverId),
    queryFn: () => listProcesses(serverId),
    refetchInterval: REFRESH_INTERVAL_MS,
    refetchOnWindowFocus: true,
  });
}

export function useTerminateProcess(serverId: number) {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (pid: number) => terminateProcess(serverId, pid),
    onSuccess: () => {
      void queryClient.invalidateQueries({
        queryKey: queryKeys.processes(serverId),
      });
    },
  });
}
