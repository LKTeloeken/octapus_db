import { memo } from 'react';
import { cn } from '@/lib/utils';
import { Spinner } from '@/components/ui/spinner';
import { Typography } from '@/components/ui/typography';
import { ResultsTable } from '@/components/query-results/results-table/results-table';

import type { ResultsContainerProps } from './results-container.types';
import { useStyles } from './results-container.styles';

export const ResultsContainer = memo(
  ({
    columns,
    rows,
    isLoading = false,
    isLoadingMore = false,
    hasMore = false,
    executionTimeMs,
    totalCount,
    rowCount,
    onLoadMore,
    error = null,
    className,
  }: ResultsContainerProps) => {
    const styles = useStyles();

    if (isLoading) {
      return (
        <div className={cn(styles.loadingContainer, className)}>
          <div className={styles.loadingContent}>
            <Spinner className={styles.loadingSpinner} />
            <Typography variant="p" className={styles.loadingText}>
              Executando query...
            </Typography>
          </div>
        </div>
      );
    }

    if (error) {
      return (
        <div className={cn(styles.errorContainer, className)}>
          <div className={styles.errorContent}>
            <Typography variant="p" className={styles.errorTitle}>
              Erro na Query
            </Typography>
            <Typography variant="p" className={styles.errorMessage}>
              {error}
            </Typography>
          </div>
        </div>
      );
    }

    if (!columns || !rows) {
      return (
        <div className={cn(styles.emptyContainer, className)}>
          <Typography variant="p" className={styles.emptyText}>
            Execute uma query para ver resultados
          </Typography>
        </div>
      );
    }

    const hasResult = rows.length > 0 || rowCount !== undefined;

    return (
      <div className={cn(styles.root, className)}>
        <div className={styles.tableWrapper}>
          <ResultsTable
            columns={columns}
            rows={rows}
            hasMore={hasMore}
            isLoadingMore={isLoadingMore}
            onLoadMore={onLoadMore}
            className="h-full"
          />
        </div>

        {hasResult && (
          <div className={styles.statusBar}>
            <div className={styles.statusBarLeft}>
              {executionTimeMs !== undefined && (
                <span>Executado em {executionTimeMs}ms</span>
              )}
              {rowCount !== undefined && (
                <span>
                  {rows.length} linhas carregadas
                  {totalCount != null && ` de ${totalCount} total`}
                </span>
              )}
            </div>
            <div className={styles.statusBarRight}>
              {isLoadingMore && (
                <>
                  <Spinner className={styles.statusBarSpinner} />
                  <span className={styles.statusBarLoadingText}>
                    Carregando...
                  </span>
                </>
              )}
              {hasMore && !isLoadingMore && (
                <span>Mais resultados disponíveis</span>
              )}
            </div>
          </div>
        )}
      </div>
    );
  },
);

ResultsContainer.displayName = 'ResultsContainer';
