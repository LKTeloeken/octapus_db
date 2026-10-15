import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { call } from './client';
import { RustCommand } from './commands';
import {
  BACKUP_EVENT,
  type BackupEvent,
  type BackupPreview,
  type BackupRequest,
  type PgToolsDetection,
  type PgToolsStatus,
  type RestoreInspection,
  type RestoreRequest,
} from './types/backup.types';

/** Procura o `pg_dump`/`pg_restore`/`psql`, sem olhar para nenhum servidor */
export function detectPgTools(): Promise<PgToolsDetection> {
  return call<PgToolsDetection>(RustCommand.PgToolsDetect);
}

/** As ferramentas frente ao servidor: versão dele e o que impede o backup */
export function getPgToolsStatus(
  serverId: number,
  database: string,
): Promise<PgToolsStatus> {
  return call<PgToolsStatus>(RustCommand.PgToolsStatus, {
    serverId,
    database,
  });
}

/** Linha de comando (sem senha) e nome sugerido, para o diálogo */
export function previewBackup(request: BackupRequest): Promise<BackupPreview> {
  return call<BackupPreview>(RustCommand.BackupPreview, { request });
}

/** Começa o pg_dump gravando em `path`; devolve o id do job */
export function startBackup(
  request: BackupRequest,
  path: string,
): Promise<string> {
  return call<string>(RustCommand.BackupStart, { request, path });
}

/** Cancela o job; o evento `cancelled` confirma */
export function cancelBackup(jobId: string): Promise<void> {
  return call<void>(RustCommand.BackupCancel, { jobId });
}

/** Formato do arquivo e, num `.dump`, o resumo do que tem dentro */
export function inspectRestore(path: string): Promise<RestoreInspection> {
  return call<RestoreInspection>(RustCommand.RestoreInspect, { path });
}

/** Linha de comando da restauração (sem senha) */
export function previewRestore(
  request: RestoreRequest,
  path: string,
): Promise<string> {
  return call<string>(RustCommand.RestorePreview, { request, path });
}

/** Começa a restauração de `path`; devolve o id do job */
export function startRestore(
  request: RestoreRequest,
  path: string,
): Promise<string> {
  return call<string>(RustCommand.RestoreStart, { request, path });
}

/** Andamento dos jobs (evento global do Tauri). Devolve o cancelamento. */
export function onBackupEvent(
  handler: (event: BackupEvent) => void,
): Promise<UnlistenFn> {
  return listen<BackupEvent>(BACKUP_EVENT, event => handler(event.payload));
}
