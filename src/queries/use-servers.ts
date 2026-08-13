import {
  useMutation,
  useQuery,
  useQueryClient,
} from '@tanstack/react-query';
import {
  createServer,
  deleteServer,
  getAllServers,
  updateServer,
} from '@/api/servers';
import type { ServerInput } from '@/api/types/server.types';
import { useConnectionStore } from '@/stores/connection-store';
import { invalidateServerScope, queryKeys, removeServerScope } from './keys';

export function useServers() {
  return useQuery({
    queryKey: queryKeys.servers,
    queryFn: getAllServers,
    staleTime: Infinity, // local SQLite — only changes through the mutations below
  });
}

export function useCreateServer() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: ServerInput) => createServer(input),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.servers });
    },
  });
}

export function useUpdateServer() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ id, input }: { id: number; input: ServerInput }) =>
      updateServer(id, input),
    onSuccess: (_server, { id }) => {
      queryClient.invalidateQueries({ queryKey: queryKeys.servers });
      // Backend drops the server's connections; host/db type may have changed
      invalidateServerScope(queryClient, id);
      useConnectionStore.getState().clearServer(id);
    },
  });
}

export function useDeleteServer() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (id: number) => deleteServer(id),
    onSuccess: (_void, id) => {
      queryClient.invalidateQueries({ queryKey: queryKeys.servers });
      removeServerScope(queryClient, id);
      useConnectionStore.getState().clearServer(id);
    },
  });
}
