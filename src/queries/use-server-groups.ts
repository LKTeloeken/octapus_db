import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  createServerGroup,
  deleteServerGroup,
  getAllServerGroups,
  renameServerGroup,
  setServerGroup,
} from '@/api/servers';
import type { Server } from '@/api/types/server.types';
import { queryKeys } from './keys';

export function useServerGroups() {
  return useQuery({
    queryKey: queryKeys.serverGroups,
    queryFn: getAllServerGroups,
    staleTime: Infinity, // SQLite local — só muda pelas mutações abaixo
  });
}

export function useCreateServerGroup() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (name: string) => createServerGroup(name),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.serverGroups });
    },
  });
}

export function useRenameServerGroup() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ id, name }: { id: number; name: string }) =>
      renameServerGroup(id, name),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.serverGroups });
    },
  });
}

export function useDeleteServerGroup() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (id: number) => deleteServerGroup(id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.serverGroups });
      // Os servidores do grupo saem dele
      queryClient.invalidateQueries({ queryKey: queryKeys.servers });
    },
  });
}

/**
 * Move um servidor de grupo. Otimista: a linha muda de lugar na árvore na
 * hora, e volta se o backend recusar.
 */
export function useSetServerGroup() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ id, groupId }: { id: number; groupId: number | null }) =>
      setServerGroup(id, groupId),
    onMutate: async ({ id, groupId }) => {
      await queryClient.cancelQueries({ queryKey: queryKeys.servers });
      const previous = queryClient.getQueryData<Server[]>(queryKeys.servers);
      queryClient.setQueryData<Server[]>(queryKeys.servers, servers =>
        servers?.map(server =>
          server.id === id ? { ...server, groupId } : server,
        ),
      );
      return { previous };
    },
    onError: (_error, _vars, context) => {
      if (context?.previous) {
        queryClient.setQueryData(queryKeys.servers, context.previous);
      }
    },
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.servers });
    },
  });
}
