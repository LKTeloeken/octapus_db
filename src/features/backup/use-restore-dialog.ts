import { open } from '@tauri-apps/plugin-dialog';
import { useCallback, useMemo, useState } from 'react';
import toast from 'react-hot-toast';
import type { RestoreOptions, RestoreRequest } from '@/api/types/backup.types';
import {
  useRestoreInspection,
  useRestorePreview,
  useStartRestore,
} from '@/queries/use-backup';
import { usePgToolsStatus } from '@/queries/use-pg-tools';
import { useBackupDialogStore, type BackupDialog } from '@/stores/backup-dialog-store';
import { useBackupJobsStore } from '@/stores/backup-jobs-store';
import {
  DEFAULT_RESTORE_OPTIONS,
  effectiveRestoreOptions,
  restoreToolsProblem,
} from './backup-options';

/** Nome do arquivo sem a pasta */
export const fileNameOf = (path: string) => path.split(/[\\/]/).at(-1) ?? path;

/**
 * Restaurar um arquivo no database: escolher o arquivo, ver o que tem dentro,
 * as opções e a linha de comando. "Apagar objetos existentes" pede uma
 * confirmação a mais antes de rodar.
 */
export function useRestoreDialog(
  dialog: Extract<BackupDialog, { kind: 'restore' }>,
) {
  const { serverId, database } = dialog;
  const close = useBackupDialogStore(state => state.close);
  const addJob = useBackupJobsStore(state => state.add);
  const [path, setPath] = useState<string | null>(null);
  const [options, setOptionsState] = useState<RestoreOptions>(
    DEFAULT_RESTORE_OPTIONS,
  );
  const [confirming, setConfirming] = useState(false);

  const setOptions = useCallback((patch: Partial<RestoreOptions>) => {
    setOptionsState(current => ({ ...current, ...patch }));
    setConfirming(false);
  }, []);

  const chooseFile = useCallback(async () => {
    const chosen = await open({
      multiple: false,
      directory: false,
      title: 'Arquivo de backup',
      filters: [
        { name: 'Backup do PostgreSQL', extensions: ['dump', 'backup', 'sql'] },
        { name: 'Todos os arquivos', extensions: ['*'] },
      ],
    });
    if (typeof chosen === 'string') {
      setPath(chosen);
      setConfirming(false);
    }
  }, []);

  const inspection = useRestoreInspection(path);
  const tools = usePgToolsStatus(serverId, database);
  const file = inspection.data ?? null;
  const format = file?.format ?? null;

  const request = useMemo<RestoreRequest | null>(
    () =>
      format
        ? {
            serverId,
            database,
            format,
            options: effectiveRestoreOptions(format, options),
          }
        : null,
    [serverId, database, format, options],
  );
  const preview = useRestorePreview(request, path);
  const start = useStartRestore();

  const toolsProblem = tools.isError
    ? tools.error.message
    : restoreToolsProblem(format, tools.data);
  // O pg_restore não leu o .dump: a restauração falharia do mesmo jeito
  const unreadable = format === 'custom' && !!file?.summaryError;

  const canStart =
    request !== null &&
    tools.isSuccess &&
    !toolsProblem &&
    !unreadable &&
    preview.isSuccess &&
    !preview.isPlaceholderData &&
    !start.isPending;

  const submit = async () => {
    if (!canStart || !request || !path) return;
    if (request.options.clean && !confirming) {
      setConfirming(true);
      return;
    }
    try {
      const jobId = await start.mutateAsync({ request, path });
      addJob({
        jobId,
        kind: 'restore',
        serverId,
        database,
        label: `${fileNameOf(path)} → ${database}`,
        path,
      });
      close();
    } catch (failure) {
      toast.error(failure instanceof Error ? failure.message : String(failure));
    }
  };

  return {
    database,
    path,
    fileName: path ? fileNameOf(path) : null,
    chooseFile,
    file,
    isInspecting: inspection.isFetching,
    inspectError: inspection.isError ? inspection.error.message : null,
    options: format ? effectiveRestoreOptions(format, options) : options,
    setOptions,
    isCheckingTools: tools.isPending,
    toolsProblem,
    preview: {
      command: preview.data ?? null,
      error: preview.isError ? preview.error.message : null,
      isLoading: preview.isFetching,
    },
    confirming,
    cancelConfirm: () => setConfirming(false),
    canStart,
    isStarting: start.isPending,
    submit,
    close,
  };
}
