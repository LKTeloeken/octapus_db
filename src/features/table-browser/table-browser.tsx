import { memo } from 'react';
import { ResultsTable } from '@/components/results-table/results-table';
import { Button } from '@/components/ui/button';
import { Typography } from '@/components/ui/typography';
import { ColumnSelector } from '@/components/column-selector/column-selector';
import { FilterBar } from './filter-bar/filter-bar';
import { useTableBrowser } from './use-table-browser';
import type { TableBrowserProps } from './table-browser.types';

/**
 * Browse view of a table/collection/key-group. All data shaping (filter,
 * sort, pagination) happens server-side via fetch_table_data — no SQL is
 * assembled on the front.
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
    fetchNextPage,
    refetch,
    setSort,
    setFilters,
    setHiddenColumns,
    save,
  } = useTableBrowser(tab);

  if (error) {
    return (
      <div className="flex flex-col h-full items-center justify-center gap-3">
        <Typography variant="p" className="text-destructive text-sm max-w-md text-center">
          {error.message}
        </Typography>
        <Button variant="outline" size="sm" onClick={refetch}>
          Tentar novamente
        </Button>
      </div>
    );
  }

  return (
    <div className="flex flex-col h-full gap-2">
      <div className="flex items-center gap-1.5 flex-wrap">
        <FilterBar columns={columns} filters={tab.filters} onChange={setFilters} />
        <ColumnSelector
          columns={columns}
          hiddenColumns={hiddenColumns}
          onChange={setHiddenColumns}
        />
      </div>

      <ResultsTable
        className="flex-1 min-h-0"
        columns={columns}
        rows={rows}
        editableInfo={editableInfo}
        hiddenColumns={hiddenColumns}
        activeSort={activeSort}
        emptyMessage={
          tab.filters.length > 0
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
    </div>
  );
});

TableBrowser.displayName = 'TableBrowser';
