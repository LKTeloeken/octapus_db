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

import type { ResultsViewMode } from '../results-table.types';
import type { DataTableStatusBarProps } from './results-table-status-bar.types';

const VIEW_MODE_OPTIONS: SegmentedControlOption<ResultsViewMode>[] = [
  { value: 'table', label: 'Tabela' },
  { value: 'vertical', label: 'Vertical' },
];

/**
 * Barra de status da grade (32 px): visualização, contagem e tempo à esquerda;
 * ações à direita. As pendências ficam na barra flutuante (ResultsTablePendingBar).
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
      <div className="flex h-8 shrink-0 items-center gap-3 border-t border-line-subtle bg-surface-1 pr-1.5 pl-2 text-small text-fg-muted">
        <SegmentedControl
          aria-label="Visualização"
          value={viewMode}
          onValueChange={onViewModeChange}
          options={VIEW_MODE_OPTIONS}
        />

        {rowCount !== undefined && (
          <span className="whitespace-nowrap tabular-nums">
            <span className="text-fg">{rowsLength}</span>
            {totalCount != null ? ` de ${totalCount}` : ''} linhas
          </span>
        )}

        {executionTimeMs !== undefined && (
          <span className="inline-flex items-center gap-1.5 font-mono text-[11px] whitespace-nowrap text-fg-subtle">
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
          <span className="whitespace-nowrap text-fg-subtle">
            Mais resultados disponíveis
          </span>
        )}

        <span className="flex-1" />

        {isEditable && (
          <Button variant="ghost" size="xs" onClick={onAddRow}>
            <HugeiconsIcon icon={Add01Icon} />
            Nova linha
          </Button>
        )}
        <Button
          variant="ghost"
          size="xs"
          disabled={rowsLength === 0}
          onClick={onExport}
        >
          <HugeiconsIcon icon={Download04Icon} />
          Exportar
        </Button>
      </div>
    );
  },
);
