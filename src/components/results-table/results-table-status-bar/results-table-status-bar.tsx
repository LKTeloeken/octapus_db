import { memo } from 'react';
import { Spinner } from '@/components/ui/spinner';
import { cn } from '@/lib/utils';

import type { DataTableStatusBarProps } from './results-table-status-bar.types';

export const DataTableStatusBar = memo(
  ({
    executionTimeMs,
    rowCount,
    rowsLength,
    totalCount,
    isEditable,
    editableInfo,
    changesCount,
    addedCount,
    removedCount,
    pendingCount,
    isLoadingMore,
    hasMore,
    viewMode,
    onViewModeChange,
    onAddRow,
    onDiscardChanges,
    onSave,
  }: DataTableStatusBarProps) => {
    return (
      <div className="flex items-center justify-between px-3 py-1.5 border-t border-border bg-purple-glow text-xs text-foreground shrink-0">
        <div className="flex items-center gap-3">
          <div className="flex items-center rounded border border-accent/25 overflow-hidden">
            <button
              type="button"
              className={cn(
                'px-2 py-0.5 transition-colors',
                viewMode === 'table'
                  ? 'bg-muted text-foreground'
                  : 'text-muted-foreground hover:bg-muted/60',
              )}
              onClick={() => onViewModeChange('table')}
            >
              Tabela
            </button>
            <button
              type="button"
              className={cn(
                'px-2 py-0.5 transition-colors',
                viewMode === 'vertical'
                  ? 'bg-muted text-foreground'
                  : 'text-muted-foreground hover:bg-muted/60',
              )}
              onClick={() => onViewModeChange('vertical')}
            >
              Vertical
            </button>
          </div>
          {executionTimeMs !== undefined && (
            <span>Executado em {executionTimeMs}ms</span>
          )}
          {rowCount !== undefined && (
            <span>
              {rowsLength} linhas carregadas
              {totalCount != null && ` de ${totalCount} total`}
            </span>
          )}
          {isEditable && editableInfo && (
            <span className="text-green-400">
              {editableInfo.schema}.{editableInfo.table}
            </span>
          )}
        </div>
        <div className="flex items-center gap-2">
          {isEditable && (
            <button
              type="button"
              className="px-2 py-0.5 text-xs rounded border border-accent/25 hover:bg-muted/60 transition-colors"
              onClick={onAddRow}
            >
              + Nova linha
            </button>
          )}
          {pendingCount > 0 && (
            <>
              <span className="flex items-center gap-2">
                {changesCount > 0 && (
                  <span className="text-yellow-400">
                    {changesCount} edição{changesCount === 1 ? '' : 'ões'}
                  </span>
                )}
                {addedCount > 0 && (
                  <span className="text-green-400">
                    {addedCount} nova{addedCount === 1 ? '' : 's'}
                  </span>
                )}
                {removedCount > 0 && (
                  <span className="text-red-400">
                    {removedCount} removida{removedCount === 1 ? '' : 's'}
                  </span>
                )}
              </span>
              <button
                type="button"
                className="px-2 py-0.5 text-xs rounded border border-accent/25 hover:bg-muted/60 transition-colors"
                onClick={onDiscardChanges}
              >
                Descartar
              </button>
              <button
                type="button"
                className="px-2 py-0.5 text-xs rounded border border-accent/25 hover:bg-muted/60 transition-colors"
                onClick={onSave}
              >
                Salvar
              </button>
            </>
          )}
          {isLoadingMore && (
            <>
              <Spinner className="h-3 w-3" />
              <span>Carregando...</span>
            </>
          )}
          {hasMore && !isLoadingMore && (
            <span>Mais resultados disponíveis</span>
          )}
        </div>
      </div>
    );
  },
);
