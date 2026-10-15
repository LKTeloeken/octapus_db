import { revealItemInDir } from '@tauri-apps/plugin-opener';
import { useCallback, useEffect, useState } from 'react';
import toast from 'react-hot-toast';
import { useCancelBackup } from '@/queries/use-backup';
import { useBackupJobsStore } from '@/stores/backup-jobs-store';

const toastError = (error: unknown) =>
  toast.error(error instanceof Error ? error.message : String(error));

/** Um job no card de progresso: tempo correndo, log, cancelar e abrir a pasta */
export function useBackupJobCard(jobId: string) {
  const job = useBackupJobsStore(state => state.jobs[jobId]);
  const dismiss = useBackupJobsStore(state => state.dismiss);
  const cancel = useCancelBackup();
  const [showLog, setShowLog] = useState(false);
  const [now, setNow] = useState(() => Date.now());

  const running = job?.status === 'running';
  useEffect(() => {
    if (!running) return;
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, [running]);

  const elapsedMs = job
    ? (job.elapsedMs ?? Math.max(0, now - job.startedAt))
    : 0;

  const cancelJob = useCallback(
    () => cancel.mutate(jobId, { onError: toastError }),
    [cancel, jobId],
  );

  const path = job?.path;
  const reveal = useCallback(() => {
    if (path) revealItemInDir(path).catch(toastError);
  }, [path]);

  const close = useCallback(() => dismiss(jobId), [dismiss, jobId]);

  return {
    job,
    elapsedMs,
    lastLine: job?.log.at(-1) ?? null,
    showLog,
    toggleLog: () => setShowLog(value => !value),
    isCancelling: cancel.isPending,
    cancelJob,
    reveal,
    close,
  };
}
