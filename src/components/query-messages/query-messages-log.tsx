import { memo, useCallback, useEffect, useRef, useState } from 'react';
import type { QueryMessage, QueryMessageKind } from '@/api/types/query.types';
import { Typography } from '@/components/ui/typography';
import { cn } from '@/lib/utils';
import type {
  QueryLogEntry,
  QueryMessagesLogProps,
} from './query-messages-log.types';

/** Distância do fim em que ainda consideramos que o usuário está acompanhando */
const AUTOSCROLL_THRESHOLD_PX = 40;

const KIND_STYLES: Record<QueryMessageKind, string> = {
  notice: 'text-sky-400 border-sky-400/30',
  warning: 'text-amber-400 border-amber-400/30',
  error: 'text-destructive border-destructive/40',
  info: 'text-muted-foreground border-border',
  status: 'text-muted-foreground border-border',
};

const formatTime = (timestampMs: number) =>
  new Date(timestampMs).toLocaleTimeString('pt-BR', { hour12: false });

/** Primeira linha não vazia da query, para dar nome ao bloco */
const querySummary = (query: string) => {
  const line = query
    .split('\n')
    .map(l => l.trim())
    .find(Boolean);

  return line ?? query.trim();
};

const MessageRow = memo(({ message }: { message: QueryMessage }) => {
  const extras = [
    message.detail && `DETAIL: ${message.detail}`,
    message.hint && `HINT: ${message.hint}`,
    message.context && `CONTEXT: ${message.context}`,
    message.sqlState && `SQLSTATE: ${message.sqlState}`,
    message.position != null && `POSITION: ${message.position}`,
  ].filter(Boolean) as string[];

  return (
    <div className="flex gap-2 px-3 py-1 font-mono text-xs leading-relaxed">
      <span className="text-muted-foreground shrink-0 tabular-nums">
        {formatTime(message.timestampMs)}
      </span>
      <span
        className={cn(
          // self-start: sem isso o flex row estica o chip até a altura do
          // bloco de texto, que pode ter várias linhas de DETAIL/CONTEXT.
          'shrink-0 self-start rounded border px-1.5 py-0.5 uppercase',
          KIND_STYLES[message.kind],
        )}
      >
        {message.severity}
      </span>
      <div className="min-w-0 flex-1">
        <p
          className={cn(
            'whitespace-pre-wrap wrap-break-word',
            message.kind === 'error' && 'text-destructive',
          )}
        >
          {message.message}
        </p>
        {extras.length > 0 && (
          <pre className="mt-0.5 whitespace-pre-wrap wrap-break-word text-muted-foreground">
            {extras.join('\n')}
          </pre>
        )}
      </div>
    </div>
  );
});

MessageRow.displayName = 'MessageRow';

const LogEntry = memo(({ entry }: { entry: QueryLogEntry }) => (
  <div className="border-b border-border/60 last:border-b-0">
    <div className="flex gap-2 bg-muted/40 px-3 py-1 font-mono text-[11px] text-muted-foreground">
      <span className="shrink-0 tabular-nums">
        {formatTime(entry.startedAt)}
      </span>
      <span className="truncate">{querySummary(entry.query)}</span>
    </div>
    {entry.messages.map((message, index) => (
      <MessageRow key={`${entry.id}-${index}`} message={message} />
    ))}
  </div>
));

LogEntry.displayName = 'LogEntry';

/**
 * Log das mensagens que o Postgres emite fora do result set. Acumula uma
 * entrada por execução da aba; o streaming vai empilhando linhas enquanto a
 * query ainda roda.
 */
export const QueryMessagesLog = memo(
  ({ className, entries, onClear }: QueryMessagesLogProps) => {
    const scrollRef = useRef<HTMLDivElement>(null);
    // Rolar para cima durante o streaming desliga o autoscroll, senão a lista
    // arrancaria o usuário de onde ele está lendo.
    const [followTail, setFollowTail] = useState(true);

    const totalMessages = entries.reduce(
      (count, entry) => count + entry.messages.length,
      0,
    );

    const handleScroll = useCallback(() => {
      const element = scrollRef.current;
      if (!element) return;

      const distanceToBottom =
        element.scrollHeight - element.scrollTop - element.clientHeight;
      setFollowTail(distanceToBottom <= AUTOSCROLL_THRESHOLD_PX);
    }, []);

    useEffect(() => {
      const element = scrollRef.current;
      if (!element || !followTail) return;

      element.scrollTop = element.scrollHeight;
    }, [followTail, totalMessages, entries.length]);

    if (entries.length === 0) {
      return (
        <div
          className={cn('flex h-full items-center justify-center', className)}
        >
          <Typography variant="p" className="text-muted-foreground">
            Nenhuma mensagem ainda
          </Typography>
        </div>
      );
    }

    return (
      <div className={cn('flex h-full flex-col', className)}>
        <div
          ref={scrollRef}
          onScroll={handleScroll}
          className="flex-1 min-h-0 overflow-auto"
        >
          {entries.map(entry => (
            <LogEntry key={entry.id} entry={entry} />
          ))}
        </div>

        <div className="flex shrink-0 items-center justify-between border-t border-border bg-purple-glow px-3 py-1.5 text-xs text-foreground">
          <span>
            {totalMessages} mensage{totalMessages === 1 ? 'm' : 'ns'} em{' '}
            {entries.length} execuç{entries.length === 1 ? 'ão' : 'ões'}
          </span>
          <button
            type="button"
            className="rounded border border-accent/25 px-2 py-0.5 transition-colors hover:bg-muted/60"
            onClick={onClear}
          >
            Limpar
          </button>
        </div>
      </div>
    );
  },
);

QueryMessagesLog.displayName = 'QueryMessagesLog';
