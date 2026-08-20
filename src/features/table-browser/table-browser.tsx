import { memo } from 'react';
import { ResultsTable } from '@/components/results-table/results-table';
import { Button } from '@/components/ui/button';
import { Typography } from '@/components/ui/typography';
import { ColumnSelector } from '@/components/column-selector/column-selector';
import { FilterInput } from './filter-input/filter-input';
import { useTableBrowser } from './use-table-browser';
import type { TableBrowserProps } from './table-browser.types';

/**
 * Browse view of a table/collection/key-group. Sort and pagination happen
 * server-side via fetch_table_data. On Postgres, an optional WHERE expression
 * is sent as `whereExpr` — the backend still owns SELECT/ORDER/LIMIT.
 */
export const TableBrowser = memo(({ tab }: TableBrowserProps) => {
  const {
    columns,
    rows,
    editableInfo,
    totalCount,
    rowCount,
    executionTimeMs,
    hasMore,
    isLoading,
    isLoadingMore,
    error,
    activeSort,
    hiddenColumns,
    draftWhere,
    setDraftWhere,
    applyWhere,
    resetWhere,
    supportsSql,
    fetchNextPage,
    setSort,
    setHiddenColumns,
    save,
  } = useTableBrowser(tab);

  return (
    <div className="flex flex-col h-full bg-sidebar rounded-md border border-border">
      <div className="flex items-center gap-1.5 flex-wrap p-2">
        <ColumnSelector
          columns={columns}
          hiddenColumns={hiddenColumns}
          onChange={setHiddenColumns}
        />

        {supportsSql && (
          <FilterInput
            value={draftWhere}
            onChange={setDraftWhere}
            onApply={applyWhere}
            onReset={resetWhere}
          />
        )}
      </div>

      {error ? (
        <div className="flex flex-col flex-1 items-center justify-center gap-3">
          <Typography
            variant="p"
            className="text-destructive text-sm max-w-md text-center"
          >
            {error.message}
          </Typography>
          <Button variant="outline" size="sm" onClick={applyWhere}>
            Tentar novamente
          </Button>
        </div>
      ) : (
        <ResultsTable
          className="flex-1 min-h-0"
          columns={columns}
          rows={rows}
          editableInfo={editableInfo}
          hiddenColumns={hiddenColumns}
          activeSort={activeSort}
          emptyMessage={
            tab.whereExpr.trim().length > 0
              ? 'Nenhum registro corresponde aos filtros'
              : 'Sem resultados na tabela'
          }
          isLoading={isLoading}
          isLoadingMore={isLoadingMore}
          hasMore={hasMore}
          executionTimeMs={executionTimeMs}
          totalCount={totalCount}
          rowCount={rowCount}
          onSort={setSort}
          onLoadMore={fetchNextPage}
          onSave={save}
        />
      )}
    </div>
  );
});

TableBrowser.displayName = 'TableBrowser';
