import { AlertCircle, Clock3, RefreshCw, Server, SquareX } from 'lucide-react';
import { memo, useMemo, useRef, useState } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import toast from 'react-hot-toast';
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Spinner } from '@/components/ui/spinner';
import { useProcesses, useTerminateProcess } from '@/queries/use-processes';
import type { DatabaseProcess } from '@/api/types/processes.types';
import type { ProcessMonitorProps } from './process-monitor.types';

const ROW_HEIGHT = 56;
const OVERSCAN = 10;

function formatDuration(durationMs: number | null): string {
  if (durationMs == null) return '—';
  if (durationMs < 1_000) return `${durationMs} ms`;

  const totalSeconds = Math.floor(durationMs / 1_000);
  if (totalSeconds < 60) return `${totalSeconds}s`;

  const minutes = Math.floor(totalSeconds / 60);
  const seconds = totalSeconds % 60;
  if (minutes < 60) return `${minutes}m ${seconds}s`;

  const hours = Math.floor(minutes / 60);
  return `${hours}h ${minutes % 60}m`;
}

function formatClient(process: DatabaseProcess): string {
  if (!process.clientAddress) return 'local';
  return process.clientPort
    ? `${process.clientAddress}:${process.clientPort}`
    : process.clientAddress;
}

function stateVariant(
  state: string | null,
): 'default' | 'secondary' | 'destructive' | 'outline' {
  if (state === 'active') return 'default';
  if (
    state === 'idle in transaction' ||
    state === 'idle in transaction (aborted)'
  ) {
    return 'destructive';
  }
  return 'secondary';
}

const EmptyProcesses = () => (
  <div className="flex h-full flex-col items-center justify-center gap-2 text-muted-foreground">
    <Server className="h-8 w-8" />
    <span>Nenhum processo visível para este usuário.</span>
  </div>
);

export const ProcessMonitor = memo(({ tab }: ProcessMonitorProps) => {
  const { data, isLoading, isError, error, refetch, isFetching } = useProcesses(
    tab.serverId,
  );
  const terminate = useTerminateProcess(tab.serverId);
  const [processToTerminate, setProcessToTerminate] =
    useState<DatabaseProcess | null>(null);
  const parentRef = useRef<HTMLDivElement>(null);
  const processes = data ?? [];

  const summary = useMemo(
    () => ({
      active: processes.filter(process => process.state === 'active').length,
      waiting: processes.filter(process => process.waitEventType != null)
        .length,
      idle: processes.filter(process => process.state?.startsWith('idle'))
        .length,
    }),
    [processes],
  );

  const virtualizer = useVirtualizer({
    count: processes.length,
    getScrollElement: () => parentRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: OVERSCAN,
    getItemKey: index => processes[index]?.pid ?? index,
  });

  const handleTerminate = async () => {
    if (!processToTerminate) return;

    const process = processToTerminate;
    setProcessToTerminate(null);
    try {
      await terminate.mutateAsync(process.pid);
      toast.success(`Processo ${process.pid} encerrado.`);
    } catch (cause) {
      toast.error(cause instanceof Error ? cause.message : String(cause));
    }
  };

  return (
    <div className="flex h-full flex-col gap-3 p-3">
      <div className="flex flex-wrap items-center justify-between gap-3">
        <div>
          <h2 className="flex items-center gap-2 text-base font-semibold">
            <Server className="h-4 w-4 text-primary" />
            Processos do servidor
          </h2>
          <p className="text-xs text-muted-foreground">
            Atividade do PostgreSQL · atualização automática a cada 5 segundos
          </p>
        </div>
        <Button
          variant="outline"
          size="sm"
          onClick={() => void refetch()}
          disabled={isFetching}
          title="Atualizar processos"
        >
          {isFetching ? <Spinner /> : <RefreshCw />}
          Atualizar
        </Button>
      </div>

      <div className="flex flex-wrap gap-2 text-xs">
        <Badge variant="default">{summary.active} ativos</Badge>
        <Badge variant="outline">{summary.waiting} aguardando evento</Badge>
        <Badge variant="secondary">{summary.idle} ociosos</Badge>
        <span className="inline-flex items-center gap-1 text-muted-foreground">
          <Clock3 className="h-3 w-3" /> {processes.length} processos visíveis
        </span>
      </div>

      {isLoading ? (
        <div className="flex flex-1 items-center justify-center">
          <Spinner />
        </div>
      ) : isError ? (
        <div className="flex flex-1 flex-col items-center justify-center gap-2 text-sm text-destructive">
          <AlertCircle className="h-5 w-5" />
          <span>{error instanceof Error ? error.message : String(error)}</span>
          <Button variant="outline" size="sm" onClick={() => void refetch()}>
            Tentar novamente
          </Button>
        </div>
      ) : processes.length === 0 ? (
        <EmptyProcesses />
      ) : (
        <div
          ref={parentRef}
          className="min-h-0 flex-1 overflow-auto rounded-md border border-border scrollbar-thin"
        >
          <div className="min-w-[1240px]">
            <div className="sticky top-0 z-10 grid h-9 grid-cols-[80px_140px_140px_170px_180px_125px_150px_105px_minmax(300px,1fr)_88px] items-center border-b border-border bg-background px-2 text-[11px] font-medium text-muted-foreground">
              <span>PID</span>
              <span>Banco</span>
              <span>Usuário</span>
              <span>Aplicação</span>
              <span>Cliente</span>
              <span>Estado</span>
              <span>Início da query</span>
              <span>Duração</span>
              <span>Query</span>
              <span>Ação</span>
            </div>
            <div
              className="relative"
              style={{ height: `${virtualizer.getTotalSize()}px` }}
            >
              {virtualizer.getVirtualItems().map(item => {
                const process = processes[item.index];
                if (!process) return null;

                return (
                  <div
                    key={item.key}
                    role="row"
                    className="absolute left-0 grid w-full grid-cols-[80px_140px_140px_170px_180px_125px_150px_105px_minmax(300px,1fr)_88px] items-center border-b border-border/70 px-2 text-xs"
                    style={{
                      height: `${item.size}px`,
                      transform: `translateY(${item.start}px)`,
                    }}
                  >
                    <code className="font-mono">{process.pid}</code>
                    <span className="truncate" title={process.database ?? '—'}>
                      {process.database ?? '—'}
                    </span>
                    <span className="truncate" title={process.username ?? '—'}>
                      {process.username ?? '—'}
                    </span>
                    <span
                      className="truncate"
                      title={process.applicationName ?? '—'}
                    >
                      {process.applicationName ?? '—'}
                    </span>
                    <span className="truncate" title={formatClient(process)}>
                      {formatClient(process)}
                    </span>
                    <span>
                      <Badge variant={stateVariant(process.state)}>
                        {process.state ?? '—'}
                      </Badge>
                    </span>
                    <span
                      className="truncate text-muted-foreground"
                      title={process.queryStart ?? '—'}
                    >
                      {process.queryStart ?? '—'}
                    </span>
                    <span className="font-mono text-muted-foreground">
                      {formatDuration(process.durationMs)}
                    </span>
                    <span className="truncate" title={process.query}>
                      {process.query || '—'}
                    </span>
                    <span>
                      <Button
                        variant="ghost"
                        size="icon"
                        className="h-7 w-7 text-destructive hover:text-destructive"
                        disabled={process.isOwnProcess || terminate.isPending}
                        onClick={() => setProcessToTerminate(process)}
                        title={
                          process.isOwnProcess
                            ? 'A conexão do monitor não pode ser encerrada'
                            : 'Encerrar processo'
                        }
                        aria-label={`Encerrar processo ${process.pid}`}
                      >
                        <SquareX />
                      </Button>
                    </span>
                  </div>
                );
              })}
            </div>
          </div>
        </div>
      )}

      <AlertDialog
        open={processToTerminate != null}
        onOpenChange={open => {
          if (!open && !terminate.isPending) setProcessToTerminate(null);
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>Encerrar processo?</AlertDialogTitle>
            <AlertDialogDescription>
              O backend PostgreSQL PID <code>{processToTerminate?.pid}</code>{' '}
              será terminado. Uma query em andamento será cancelada e a
              transação aberta poderá sofrer rollback.
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={terminate.isPending}>
              Cancelar
            </AlertDialogCancel>
            <AlertDialogAction
              className="bg-destructive text-white hover:bg-destructive/90"
              disabled={terminate.isPending}
              onClick={event => {
                event.preventDefault();
                void handleTerminate();
              }}
            >
              {terminate.isPending && <Spinner />}
              Encerrar processo
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
});

ProcessMonitor.displayName = 'ProcessMonitor';
