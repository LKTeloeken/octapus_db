import {
  Copy01Icon,
  FileCodeIcon,
  PencilEdit02Icon,
  Refresh01Icon,
  ServerStack01Icon,
} from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import { memo } from 'react';
import { ErrorDetails } from '@/components/error-details/error-details';
import { QueryEditor } from '@/components/query-editor/query-editor/query-editor';
import { LoadingState } from '@/components/results-table/loading-state';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { DB_TYPE_TEXT_COLOR } from '@/lib/db-defaults';
import { cn } from '@/lib/utils';
import type { DefinitionTab as DefinitionTabState } from '@/stores/tabs-store';
import { useDefinitionTab } from './use-definition-tab';

const noop = () => {};

/**
 * Aba de definição (índice, função, view…): só o texto, com destaque de
 * sintaxe e sem executar. A ação da área é levar o texto a uma aba de query.
 */
export const DefinitionTab = memo(({ tab }: { tab: DefinitionTabState }) => {
  const {
    text,
    isLoading,
    error,
    retry,
    kindLabel,
    serverName,
    dbType,
    dialect,
    openInQuery,
    copy,
  } = useDefinitionTab(tab);

  return (
    <div className="flex h-full flex-col overflow-hidden rounded-lg border border-line bg-surface-1">
      <div className="flex h-11 shrink-0 items-center gap-1.5 border-b border-line-subtle px-2">
        <Button size="sm" onClick={openInQuery} disabled={!text}>
          <HugeiconsIcon icon={PencilEdit02Icon} />
          Abrir em aba de query
        </Button>
        <Button
          variant="ghost"
          size="sm"
          onClick={() => void copy()}
          disabled={!text}
        >
          <HugeiconsIcon icon={Copy01Icon} />
          Copiar
        </Button>

        <span aria-hidden className="mx-1 h-4 w-px shrink-0 bg-line" />

        <Badge>{kindLabel}</Badge>
        <span className="inline-flex min-w-0 items-center gap-[7px] px-1 text-body text-fg">
          <HugeiconsIcon
            icon={FileCodeIcon}
            className="size-[15px] shrink-0 text-fg-muted"
          />
          <span className="truncate">{tab.title}</span>
        </span>

        <span className="flex-1" />

        <span className="inline-flex min-w-0 items-center gap-[7px] px-1 text-small text-fg-muted">
          <HugeiconsIcon
            icon={ServerStack01Icon}
            className={cn(
              'size-[14px] shrink-0',
              dbType ? DB_TYPE_TEXT_COLOR[dbType] : 'text-fg-subtle',
            )}
          />
          <span className="truncate">{serverName}</span>
          <span className="text-fg-subtle">/</span>
          <span className="truncate">{tab.database}</span>
        </span>
        <Button
          variant="ghost"
          size="icon"
          aria-label="Recarregar a definição"
          title="Recarregar a definição"
          onClick={retry}
        >
          <HugeiconsIcon icon={Refresh01Icon} />
        </Button>
      </div>

      {isLoading ? (
        <LoadingState className="flex-1" />
      ) : error ? (
        <div className="p-4">
          <ErrorDetails
            details={error}
            actions={
              <Button variant="ghost" size="xs" onClick={retry}>
                <HugeiconsIcon icon={Refresh01Icon} />
                Tentar de novo
              </Button>
            }
          />
        </div>
      ) : (
        <QueryEditor
          className="min-h-0 flex-1"
          height="100%"
          value={text}
          documentId={tab.id}
          dialect={dialect}
          onChange={noop}
          readOnly
        />
      )}
    </div>
  );
});

DefinitionTab.displayName = 'DefinitionTab';
