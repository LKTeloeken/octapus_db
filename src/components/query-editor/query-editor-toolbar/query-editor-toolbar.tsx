import { HugeiconsIcon } from '@hugeicons/react';
import { PlayIcon, ServerStack01Icon } from '@hugeicons/core-free-icons';
import { memo, type FC } from 'react';
import { Button } from '@/components/ui/button';
import { Kbd } from '@/components/ui/kbd';
import { Spinner } from '@/components/ui/spinner';
import { DB_TYPE_TEXT_COLOR } from '@/lib/db-defaults';
import { shortcut } from '@/lib/platform';
import { cn } from '@/lib/utils';
import type { QueryEditorToolbarProps } from './query-editor-toolbar.types';

/**
 * Toolbar do editor (44 px): Executar é a ação primária da área (Iris); ao
 * lado, o servidor/banco onde a consulta roda.
 */
export const QueryEditorToolbar: FC<QueryEditorToolbarProps> = memo(
  ({ onRun, isLoading = false, disabled = false, serverName, database, dbType }) => {
    return (
      <div className="flex h-11 shrink-0 items-center gap-1.5 border-b border-line-subtle px-2">
        <Button size="sm" onClick={onRun} disabled={disabled || isLoading}>
          {isLoading ? (
            <Spinner className="size-4" />
          ) : (
            <HugeiconsIcon icon={PlayIcon} />
          )}
          Executar
          <Kbd variant="inverse">
            {shortcut('↵')}
          </Kbd>
        </Button>

        <span aria-hidden className="mx-1 h-4 w-px shrink-0 bg-line" />

        <span className="inline-flex min-w-0 items-center gap-[7px] px-1 text-body text-fg">
          <HugeiconsIcon
            icon={ServerStack01Icon}
            className={cn(
              'size-[15px] shrink-0',
              dbType ? DB_TYPE_TEXT_COLOR[dbType] : 'text-fg-subtle',
            )}
          />
          <span className="truncate">{serverName}</span>
          <span className="text-fg-subtle">/</span>
          <span className="truncate">{database}</span>
        </span>

        <span className="flex-1" />

        <span className="shrink-0 pr-1 text-small text-fg-subtle">
          Seleção executa só o trecho
        </span>
      </div>
    );
  },
);

QueryEditorToolbar.displayName = 'QueryEditorToolbar';
