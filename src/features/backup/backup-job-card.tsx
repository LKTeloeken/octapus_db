import {
  Alert02Icon,
  Cancel01Icon,
  CheckmarkCircle02Icon,
} from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import { useEffect, useRef } from 'react';
import { Button } from '@/components/ui/button';
import { Spinner } from '@/components/ui/spinner';
import { formatBytes } from '@/lib/format-bytes';
import { cn } from '@/lib/utils';
import type {
  BackupJobKind,
  BackupJobStatus,
} from '@/stores/backup-jobs-store';
import { useBackupJobCard } from './use-backup-job-card';

/** `4 s`, `1 min 12 s` */
function formatElapsed(ms: number): string {
  const seconds = Math.round(ms / 1000);
  if (seconds < 60) return `${seconds} s`;
  return `${Math.floor(seconds / 60)} min ${seconds % 60} s`;
}

const TITLES: Record<BackupJobKind, Record<BackupJobStatus, string>> = {
  backup: {
    running: 'Fazendo backup…',
    done: 'Backup concluído',
    error: 'O backup falhou',
    cancelled: 'Backup cancelado',
  },
  restore: {
    running: 'Restaurando…',
    done: 'Restauração concluída',
    error: 'A restauração falhou',
    cancelled: 'Restauração cancelada',
  },
};

/** O card de um backup ou restauração no canto da tela (um toast em vidro) */
export function BackupJobCard({ jobId }: { jobId: string }) {
  const {
    job,
    elapsedMs,
    lastLine,
    showLog,
    toggleLog,
    isCancelling,
    cancelJob,
    reveal,
    close,
  } = useBackupJobCard(jobId);
  const logRef = useRef<HTMLPreElement>(null);

  // O log aberto acompanha o fim, como um terminal
  const logLength = job?.log.length ?? 0;
  useEffect(() => {
    const pre = logRef.current;
    if (pre) pre.scrollTop = pre.scrollHeight;
  }, [logLength, showLog]);

  if (!job) return null;

  const fileName = job.path.split(/[\\/]/).at(-1) ?? job.path;
  const restore = job.kind === 'restore';
  const warned = job.status === 'done' && job.warnings.length > 0;
  const title = TITLES[job.kind][job.status] + (warned ? ' com avisos' : '');

  return (
    <div className="glass flex w-80 flex-col gap-2.5 rounded-lg p-4 text-fg">
      <div className="flex items-start gap-2.5">
        <span className="mt-0.5 shrink-0">
          {job.status === 'running' && (
            <Spinner className="size-4 text-fg-muted" />
          )}
          {job.status === 'done' && (
            <HugeiconsIcon
              icon={warned ? Alert02Icon : CheckmarkCircle02Icon}
              className={cn('size-4', warned ? 'text-warning' : 'text-success')}
            />
          )}
          {job.status === 'error' && (
            <HugeiconsIcon icon={Alert02Icon} className="size-4 text-danger" />
          )}
          {job.status === 'cancelled' && (
            <HugeiconsIcon
              icon={Cancel01Icon}
              className="size-4 text-fg-muted"
            />
          )}
        </span>
        <div className="flex min-w-0 flex-1 flex-col gap-0.5">
          <p className="text-body font-medium">{title}</p>
          <p
            className="truncate font-mono text-small text-fg-muted"
            title={job.label}
          >
            {job.label}
          </p>
          <p className="text-small text-fg-muted">
            {job.status === 'done' && restore
              ? `Em ${formatElapsed(elapsedMs)}`
              : job.status === 'done' && job.sizeBytes !== null
                ? `${fileName} · ${formatBytes(job.sizeBytes)} em ${formatElapsed(elapsedMs)}`
                : job.status === 'running'
                  ? formatElapsed(elapsedMs)
                  : null}
          </p>
        </div>
      </div>

      {job.status === 'running' && lastLine && !showLog && (
        <p
          className="truncate font-mono text-small text-fg-muted"
          title={lastLine}
        >
          {lastLine}
        </p>
      )}
      {job.status === 'error' && job.error && (
        <p className="max-h-24 overflow-auto text-small break-words whitespace-pre-wrap text-danger scrollbar-thin">
          {job.error}
        </p>
      )}

      {warned && (
        <div className="flex flex-col gap-1">
          <p className="text-small text-fg-muted">
            {job.warnings.length === 1
              ? '1 erro foi ignorado:'
              : `${job.warnings.length} erros foram ignorados:`}
          </p>
          <pre className="max-h-24 overflow-auto rounded-sm border border-line bg-field px-2 py-1.5 font-mono text-small whitespace-pre-wrap break-words text-fg-muted scrollbar-thin">
            {job.warnings.join('\n')}
          </pre>
        </div>
      )}

      {showLog && (
        <pre
          ref={logRef}
          className={cn(
            'max-h-40 overflow-auto rounded-sm border border-line bg-field px-2 py-1.5 font-mono text-small whitespace-pre-wrap break-all text-fg-muted scrollbar-thin',
          )}
        >
          {job.log.length ? job.log.join('\n') : 'Sem mensagens ainda.'}
        </pre>
      )}

      <div className="flex justify-end gap-1.5">
        <Button variant="ghost" size="xs" onClick={toggleLog}>
          {showLog ? 'Esconder log' : 'Ver log'}
        </Button>
        {job.status === 'running' ? (
          <Button
            variant="outline"
            size="xs"
            disabled={isCancelling}
            onClick={cancelJob}
          >
            Cancelar
          </Button>
        ) : (
          <>
            {job.status === 'done' && !restore && (
              <Button variant="outline" size="xs" onClick={reveal}>
                Mostrar na pasta
              </Button>
            )}
            <Button variant="ghost" size="xs" onClick={close}>
              Fechar
            </Button>
          </>
        )}
      </div>
    </div>
  );
}
