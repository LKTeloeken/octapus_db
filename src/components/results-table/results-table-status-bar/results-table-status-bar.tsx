import { HugeiconsIcon } from '@hugeicons/react';
import {
  Add01Icon,
  Clock01Icon,
  Download04Icon,
} from '@hugeicons/core-free-icons';
import { memo } from 'react';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { SegmentedControl } from '@/components/ui/segmented-control/segmented-control';
import type { SegmentedControlOption } from '@/components/ui/segmented-control/segmented-control.types';
import { Spinner } from '@/components/ui/spinner';
import { Tooltip } from '@/components/ui/tooltip/tooltip';

import type { ResultsViewMode } from '../results-table.types';
import type { DataTableStatusBarProps } from './results-table-status-bar.types';

const VIEW_MODE_OPTIONS: SegmentedControlOption<ResultsViewMode>[] = [
  { value: 'table', label: 'Tabela' },
  { value: 'vertical', label: 'Vertical' },
];

/**
 * Barra de status da grade (32 px): visualização, contagem e tempo à esquerda;
 * ações à direita. As pendências ficam na barra flutuante (ResultsTablePendingBar).
 * Numa grade estreita (container query) somem o aviso de "mais resultados" e o
 * tempo, e as ações ficam só com o ícone.
 */
export const DataTableStatusBar = memo(
  ({
    executionTimeMs,
    rowCount,
    rowsLength,
    totalCount,
    isEditable,
    isLoadingMore,
    hasMore,
    viewMode,
    onViewModeChange,
    onAddRow,
    onExport,
  }: DataTableStatusBarProps) => {
    return (
      <div className="@container flex h-8 shrink-0 items-center gap-3 border-t border-line-subtle bg-surface-1 pr-1.5 pl-2 text-small text-fg-muted">
        <Tooltip content="Trocar visualização (Tab nos resultados)">
          <SegmentedControl
            aria-label="Visualização"
            value={viewMode}
            onValueChange={onViewModeChange}
            options={VIEW_MODE_OPTIONS}
          />
        </Tooltip>

        {rowCount !== undefined && (
          <span className="whitespace-nowrap tabular-nums">
            <span className="text-fg">{rowsLength}</span>
            {totalCount != null ? ` de ${totalCount}` : ''} linhas
          </span>
        )}

        {executionTimeMs !== undefined && (
          <span className="inline-flex items-center gap-1.5 font-mono text-[11px] whitespace-nowrap text-fg-subtle @max-md:hidden">
            <HugeiconsIcon icon={Clock01Icon} className="size-[13px]" />
            {executionTimeMs} ms
          </span>
        )}

        {!isEditable && rowsLength > 0 && <Badge>Somente leitura</Badge>}

        {isLoadingMore && (
          <span className="inline-flex items-center gap-1.5 text-fg-subtle">
            <Spinner className="size-3" />
            Carregando…
          </span>
        )}
        {hasMore && !isLoadingMore && (
          <span className="whitespace-nowrap text-fg-subtle @max-2xl:hidden">
            Mais resultados disponíveis
          </span>
        )}

        <span className="flex-1" />

        {isEditable && (
          <Tooltip content="Nova linha">
            <Button variant="ghost" size="xs" onClick={onAddRow}>
              <HugeiconsIcon icon={Add01Icon} />
              <span className="@max-xl:sr-only">Nova linha</span>
            </Button>
          </Tooltip>
        )}
        <Tooltip content="Exportar">
          <Button
            variant="ghost"
            size="xs"
            disabled={rowsLength === 0}
            onClick={onExport}
          >
            <HugeiconsIcon icon={Download04Icon} />
            <span className="@max-xl:sr-only">Exportar</span>
          </Button>
        </Tooltip>
      </div>
    );
  },
);
