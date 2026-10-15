import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import {
  commitImport,
  detectImportSources,
  discardImport,
  previewImport,
} from '@/api/import';
import type {
  ImportCommitRequest,
  ImportPreviewRequest,
} from '@/api/types/import.types';
import { queryKeys } from './keys';

/** Detecta os apps ao abrir o diálogo — sempre de novo (o usuário pode ter instalado algo) */
export function useImportSources(enabled: boolean) {
  return useQuery({
    queryKey: queryKeys.importSources,
    queryFn: detectImportSources,
    enabled,
    staleTime: 0,
    gcTime: 0,
  });
}

/** Mutação, não query: cada leitura gera um token novo no backend */
export function useImportPreview() {
  return useMutation({
    mutationFn: (request: ImportPreviewRequest) => previewImport(request),
  });
}

export function useImportCommit() {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (request: ImportCommitRequest) => commitImport(request),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: queryKeys.servers });
      queryClient.invalidateQueries({ queryKey: queryKeys.serverGroups });
    },
  });
}

export function useImportDiscard() {
  return useMutation({ mutationFn: discardImport });
}
