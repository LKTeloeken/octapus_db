import { memo } from 'react';
import { Spinner } from '@/components/ui/spinner';

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
    isLoadingMore,
    hasMore,
    onDiscardChanges,
    onApplyChanges,
  }: DataTableStatusBarProps) => {
    return (
      <div className="flex items-center justify-between px-3 py-1.5 border-t border-border bg-purple-glow text-xs text-foreground shrink-0">
        <div className="flex items-center gap-3">
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
          {changesCount > 0 && (
            <>
              <span className="text-yellow-400">
                {changesCount} alteraç{changesCount === 1 ? 'ão' : 'ões'}{' '}
                pendente
                {changesCount === 1 ? '' : 's'}
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
                onClick={onApplyChanges}
              >
                Aplicar
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
