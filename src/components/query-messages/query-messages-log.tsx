import { memo, useCallback, useEffect, useRef, useState } from 'react';
import { Copy01Icon } from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import type { QueryMessage, QueryMessageKind } from '@/api/types/query.types';
import { Button } from '@/components/ui/button';
import { Tooltip } from '@/components/ui/tooltip/tooltip';
import { cn } from '@/lib/utils';
import type {
  QueryLogEntry,
  QueryMessagesLogProps,
} from './query-messages-log.types';
import {
  formatEntryText,
  formatLogText,
  messageExtras,
} from './query-messages-text';
import { useCopyLogText } from './use-copy-log-text';

/** Distância do fim em que ainda consideramos que o usuário está acompanhando */
const AUTOSCROLL_THRESHOLD_PX = 40;

// Mesmos tons dos badges: NOTICE em info, WARNING em warning, ERROR em danger.
const KIND_STYLES: Record<QueryMessageKind, string> = {
  notice: 'bg-info-soft text-info',
  warning: 'bg-warning-soft text-warning',
  error: 'bg-danger-soft text-danger',
  info: 'bg-hover text-fg-muted',
  status: 'bg-hover text-fg-muted',
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

/**
 * Seleção com o mouse: só o texto da mensagem é selecionável. Hora, chip,
 * cabeçalho da execução e as linhas de DETAIL/CONTEXT/SQLSTATE… são
 * `select-none` — continuam na tela, mas arrastar sobre o log (ou os botões
 * Copiar) leva só as mensagens.
 */
const MessageRow = memo(({ message }: { message: QueryMessage }) => {
  const extras = messageExtras(message);

  return (
    <div className="flex gap-2.5 px-3.5 py-1 font-mono text-small leading-5">
      <span className="shrink-0 select-none tabular-nums text-fg-subtle">
        {formatTime(message.timestampMs)}
      </span>
      <span
        className={cn(
          // self-start: sem isso o flex row estica o chip até a altura do
          // bloco de texto, que pode ter várias linhas de DETAIL/CONTEXT.
          'mt-px inline-flex h-[18px] shrink-0 select-none items-center self-start rounded-[5px] px-1.5 text-[11px] font-medium uppercase',
          KIND_STYLES[message.kind],
        )}
      >
        {message.severity}
      </span>
      <div className="min-w-0 flex-1">
        <p
          className={cn(
            'whitespace-pre-wrap wrap-break-word select-text',
            message.kind === 'error' ? 'text-danger' : 'text-fg',
          )}
        >
          {message.message}
        </p>
        {extras.length > 0 && (
          <pre className="mt-0.5 select-none whitespace-pre-wrap wrap-break-word text-fg-muted">
            {extras.join('\n')}
          </pre>
        )}
      </div>
    </div>
  );
});

MessageRow.displayName = 'MessageRow';

const LogEntry = memo(
  ({
    entry,
    onCopy,
  }: {
    entry: QueryLogEntry;
    onCopy: (entry: QueryLogEntry) => void;
  }) => (
    <div className="border-b border-line-subtle last:border-b-0">
      <div className="flex h-7 select-none items-center gap-2.5 bg-surface-2 pr-1 pl-3.5 font-mono text-[11px] text-fg-subtle">
        <span className="shrink-0 tabular-nums">
          {formatTime(entry.startedAt)}
        </span>
        <span className="min-w-0 flex-1 truncate">
          {querySummary(entry.query)}
        </span>
        {entry.messages.length > 0 && (
          <Tooltip content="Copiar mensagens" position="left">
            <Button
              variant="ghost"
              size="icon-xs"
              aria-label="Copiar mensagens desta execução"
              onClick={() => onCopy(entry)}
            >
              <HugeiconsIcon icon={Copy01Icon} />
            </Button>
          </Tooltip>
        )}
      </div>
      {entry.messages.map((message, index) => (
        <MessageRow key={`${entry.id}-${index}`} message={message} />
      ))}
    </div>
  ),
);

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
    const copyText = useCopyLogText();

    const copyEntry = useCallback(
      (entry: QueryLogEntry) =>
        void copyText(formatEntryText(entry), 'Mensagens copiadas'),
      [copyText],
    );
    const copyAll = useCallback(
      () => void copyText(formatLogText(entries), 'Log copiado'),
      [copyText, entries],
    );

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
          className={cn(
            'flex h-full items-center justify-center text-body text-fg-subtle',
            className,
          )}
        >
          Nenhuma mensagem ainda
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
            <LogEntry key={entry.id} entry={entry} onCopy={copyEntry} />
          ))}
        </div>

        <div className="flex h-8 shrink-0 select-none items-center justify-between border-t border-line-subtle bg-surface-1 pr-1.5 pl-3.5 text-small text-fg-muted">
          <span>
            {totalMessages} mensage{totalMessages === 1 ? 'm' : 'ns'} em{' '}
            {entries.length} execuç{entries.length === 1 ? 'ão' : 'ões'}
          </span>
          <div className="flex items-center gap-1">
            <Button
              variant="ghost"
              size="xs"
              onClick={copyAll}
              disabled={totalMessages === 0}
            >
              Copiar tudo
            </Button>
            <Button variant="ghost" size="xs" onClick={onClear}>
              Limpar
            </Button>
          </div>
        </div>
      </div>
    );
  },
);

QueryMessagesLog.displayName = 'QueryMessagesLog';
