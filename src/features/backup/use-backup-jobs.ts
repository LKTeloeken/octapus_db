import { useQueryClient } from '@tanstack/react-query';
import { createElement, useEffect, useRef } from 'react';
import toast from 'react-hot-toast';
import { invalidateDatabaseScope } from '@/queries/keys';
import { useBackupEvents } from '@/queries/use-backup-events';
import { useBackupJobsStore, type BackupJobStatus } from '@/stores/backup-jobs-store';
import { BackupJobCard } from './backup-job-card';

/** Um job cancelado some sozinho; os outros esperam o usuário fechar */
const CANCELLED_DISMISS_MS = 4000;

/**
 * Um toast por job, que vive enquanto o job está na lista. O card lê o job do
 * store, então o toast é criado uma vez só e se atualiza sozinho.
 */
export function useBackupJobs() {
  useBackupEvents();
  const jobs = useBackupJobsStore(state => state.jobs);
  const dismiss = useBackupJobsStore(state => state.dismiss);
  const queryClient = useQueryClient();
  const shown = useRef(new Map<string, BackupJobStatus>());

  useEffect(() => {
    for (const job of Object.values(jobs)) {
      const previous = shown.current.get(job.jobId);
      if (previous === job.status) continue;
      shown.current.set(job.jobId, job.status);

      if (previous === undefined) {
        toast.custom(createElement(BackupJobCard, { jobId: job.jobId }), {
          id: job.jobId,
          duration: Infinity,
          position: 'bottom-right',
        });
      }
      // Uma restauração mexe no database (mesmo a que falhou no meio, se não
      // era "tudo ou nada"): colunas e dados em cache saem; a árvore volta
      // pelo `catalog-event` da revalidação que o backend dispara
      if (
        job.kind === 'restore' &&
        (job.status === 'done' || job.status === 'error')
      ) {
        void invalidateDatabaseScope(queryClient, job.serverId, job.database);
      }
      if (job.status === 'cancelled') {
        window.setTimeout(() => dismiss(job.jobId), CANCELLED_DISMISS_MS);
      }
    }
    // Saiu da lista (fechado): o toast vai junto
    for (const jobId of Array.from(shown.current.keys())) {
      if (!jobs[jobId]) {
        shown.current.delete(jobId);
        toast.dismiss(jobId);
      }
    }
  }, [jobs, dismiss, queryClient]);
}
