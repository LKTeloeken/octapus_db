import { create } from 'zustand';
import type { BackupEvent } from '@/api/types/backup.types';

/** O log guardado por job — o `--verbose` de um database grande passa disso */
export const JOB_LOG_LIMIT = 1000;

export type BackupJobStatus = 'running' | 'done' | 'error' | 'cancelled';

export type BackupJobKind = 'backup' | 'restore';

export interface BackupJob {
  jobId: string;
  kind: BackupJobKind;
  /** Onde o job roda (a restauração invalida o cache desse database) */
  serverId: number;
  database: string;
  /** O que está sendo salvo (`loja › public.pedidos`) */
  label: string;
  /** Arquivo gerado (backup) ou lido (restauração) */
  path: string;
  status: BackupJobStatus;
  /** As últimas `JOB_LOG_LIMIT` linhas do log */
  log: string[];
  startedAt: number;
  sizeBytes: number | null;
  elapsedMs: number | null;
  error: string | null;
  /** Erros que a restauração ignorou e seguiu */
  warnings: string[];
}

export type NewBackupJob = Pick<
  BackupJob,
  'jobId' | 'kind' | 'serverId' | 'database' | 'label' | 'path'
>;

interface BackupJobsState {
  /** Jobs por id, na ordem em que começaram */
  jobs: Record<string, BackupJob>;
  /** Registra o job que o backend acabou de começar */
  add: (job: NewBackupJob) => void;
  /** Aplica um `backup-event` */
  apply: (event: BackupEvent) => void;
  /** Tira da lista (o card foi fechado) */
  dismiss: (jobId: string) => void;
}

const emptyJob = (jobId: string): BackupJob => ({
  jobId,
  kind: 'backup',
  serverId: 0,
  database: '',
  label: '',
  path: '',
  status: 'running',
  log: [],
  startedAt: Date.now(),
  sizeBytes: null,
  elapsedMs: null,
  error: null,
  warnings: [],
});

/** O job depois do evento (puro, testável) */
export function applyBackupEvent(job: BackupJob, event: BackupEvent): BackupJob {
  switch (event.type) {
    case 'log': {
      const log = job.log.concat(event.lines);
      return {
        ...job,
        log: log.length > JOB_LOG_LIMIT ? log.slice(-JOB_LOG_LIMIT) : log,
      };
    }
    case 'done':
      return {
        ...job,
        status: 'done',
        path: event.path || job.path,
        sizeBytes: event.sizeBytes,
        elapsedMs: event.elapsedMs,
        warnings: event.warnings,
      };
    case 'error':
      return { ...job, status: 'error', error: event.message };
    case 'cancelled':
      return { ...job, status: 'cancelled' };
  }
}

/**
 * Backups em andamento e recém-terminados (estado de UI — o backend não
 * guarda histórico). Um evento pode chegar antes do `add` (o job falhou antes
 * de o `backup_start` voltar): o job nasce vazio e o `add` completa.
 */
export const useBackupJobsStore = create<BackupJobsState>(set => ({
  jobs: {},
  add: job =>
    set(state => {
      const current = state.jobs[job.jobId] ?? emptyJob(job.jobId);
      return {
        jobs: {
          ...state.jobs,
          [job.jobId]: { ...current, ...job, path: current.path || job.path },
        },
      };
    }),
  apply: event =>
    set(state => ({
      jobs: {
        ...state.jobs,
        [event.jobId]: applyBackupEvent(
          state.jobs[event.jobId] ?? emptyJob(event.jobId),
          event,
        ),
      },
    })),
  dismiss: jobId =>
    set(state => {
      const { [jobId]: _removed, ...jobs } = state.jobs;
      return { jobs };
    }),
}));
