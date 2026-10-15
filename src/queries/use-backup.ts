import { keepPreviousData, useMutation, useQuery } from '@tanstack/react-query';
import {
  cancelBackup,
  inspectRestore,
  previewBackup,
  previewRestore,
  startBackup,
  startRestore,
} from '@/api/backup';
import type { BackupRequest, RestoreRequest } from '@/api/types/backup.types';
import { queryKeys } from './keys';

/**
 * A linha de comando e o nome sugerido para o pedido atual. Barato (não
 * decifra senha nem procura binário): cada combinação de opções é uma key.
 */
export function useBackupPreview(request: BackupRequest | null) {
  return useQuery({
    queryKey: queryKeys.backupPreview(request),
    queryFn: () => previewBackup(request!),
    enabled: request !== null,
    // Enquanto a prévia nova não chega, a anterior fica (sem piscar)
    placeholderData: keepPreviousData,
    retry: false,
    staleTime: 0,
    gcTime: 30_000,
  });
}

export interface StartBackupVariables {
  request: BackupRequest;
  path: string;
}

/** Começa o backup; o andamento chega pelo `backup-event` */
export function useStartBackup() {
  return useMutation({
    mutationFn: ({ request, path }: StartBackupVariables) =>
      startBackup(request, path),
  });
}

export function useCancelBackup() {
  return useMutation({ mutationFn: cancelBackup });
}

/** O arquivo escolhido para restaurar (formato e resumo do `.dump`) */
export function useRestoreInspection(path: string | null) {
  return useQuery({
    queryKey: queryKeys.restoreInspection(path),
    queryFn: () => inspectRestore(path!),
    enabled: path !== null,
    retry: false,
    staleTime: 0,
    gcTime: 0,
  });
}

/** A linha de comando da restauração para as opções atuais */
export function useRestorePreview(
  request: RestoreRequest | null,
  path: string | null,
) {
  return useQuery({
    queryKey: queryKeys.restorePreview(request, path),
    queryFn: () => previewRestore(request!, path!),
    enabled: request !== null && path !== null,
    placeholderData: keepPreviousData,
    retry: false,
    staleTime: 0,
    gcTime: 30_000,
  });
}

export interface StartRestoreVariables {
  request: RestoreRequest;
  path: string;
}

/** Começa a restauração; o andamento chega pelo `backup-event` */
export function useStartRestore() {
  return useMutation({
    mutationFn: ({ request, path }: StartRestoreVariables) =>
      startRestore(request, path),
  });
}
