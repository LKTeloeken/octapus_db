import { save } from '@tauri-apps/plugin-dialog';
import { useCallback, useMemo, useState } from 'react';
import toast from 'react-hot-toast';
import type { BackupOptions, BackupRequest } from '@/api/types/backup.types';
import { describePgToolsProblem } from '@/lib/pg-tools';
import { useBackupPreview, useStartBackup } from '@/queries/use-backup';
import { usePgToolsStatus } from '@/queries/use-pg-tools';
import { useBackupDialogStore, type BackupDialog } from '@/stores/backup-dialog-store';
import { useBackupJobsStore } from '@/stores/backup-jobs-store';
import {
  DEFAULT_BACKUP_OPTIONS,
  backupTargetLabel,
  normalizeBackupOptions,
} from './backup-options';

/**
 * Backup de um database, schema ou tabela: opções, a linha de comando que vai
 * rodar e o estado das ferramentas. Ao iniciar, o diálogo fecha e o andamento
 * segue no card de progresso.
 */
export function useBackupDialog(
  dialog: Extract<BackupDialog, { kind: 'backup' }>,
) {
  const { serverId, database, target } = dialog;
  const close = useBackupDialogStore(state => state.close);
  const addJob = useBackupJobsStore(state => state.add);
  const [options, setOptionsState] = useState<BackupOptions>(
    DEFAULT_BACKUP_OPTIONS,
  );

  const setOptions = useCallback(
    (patch: Partial<BackupOptions>) =>
      setOptionsState(current => normalizeBackupOptions({ ...current, ...patch })),
    [],
  );

  const request = useMemo<BackupRequest>(
    () => ({ serverId, database, target, options }),
    [serverId, database, target, options],
  );

  const tools = usePgToolsStatus(serverId, database);
  const preview = useBackupPreview(request);
  const start = useStartBackup();

  const problem = tools.data?.problem ?? null;
  const toolsError = tools.isError ? tools.error.message : null;
  const previewError = preview.isError ? preview.error.message : null;

  const canStart =
    tools.isSuccess &&
    problem === null &&
    preview.isSuccess &&
    !preview.isPlaceholderData &&
    !start.isPending;

  const submit = async () => {
    if (!canStart || !preview.data) return;
    const extension = options.format === 'custom' ? 'dump' : 'sql';
    const path = await save({
      defaultPath: preview.data.fileName,
      filters: [
        {
          name: options.format === 'custom' ? 'Backup do PostgreSQL' : 'Script SQL',
          extensions: [extension],
        },
      ],
    });
    if (!path) return;

    try {
      const jobId = await start.mutateAsync({ request, path });
      addJob({
        jobId,
        kind: 'backup',
        serverId,
        database,
        label: backupTargetLabel(database, target),
        path,
      });
      close();
    } catch (failure) {
      toast.error(failure instanceof Error ? failure.message : String(failure));
    }
  };

  return {
    options,
    setOptions,
    title: backupTargetLabel(database, target),
    isTable: target.kind === 'table',
    tools: {
      isLoading: tools.isPending,
      error: toolsError,
      problem: problem ? describePgToolsProblem(problem) : null,
      clientVersion: tools.data?.selected?.version ?? null,
      serverVersion: tools.data?.serverVersion ?? null,
    },
    preview: {
      command: preview.data?.command ?? null,
      error: previewError,
      isLoading: preview.isFetching,
    },
    canStart,
    isStarting: start.isPending,
    submit,
    close,
  };
}
