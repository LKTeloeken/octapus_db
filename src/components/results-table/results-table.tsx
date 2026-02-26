import { cn } from '@/lib/utils';
import type { ResultsTableProps } from './results-table.types';
import { memo, useCallback, useEffect, useRef } from 'react';
import ColumnCell from './results-table-column-cell/results-table-column-cell';
import useResultsTable from './use-results-table';
import { useVirtualizer } from '@tanstack/react-virtual';
import { DataTableStatusBar } from './results-table-status-bar/results-table-status-bar';
import { EmptyState } from './empty-state';
import { LodingState } from './loding-state';
import { ResultsTableRowCell } from './results-table-row-cell/results-table-row-cell';

const ROW_HEIGHT = 32;
const OVERSCAN = 10;

export const ResultsTable = memo(
  ({
    columns,
    rows,
    onLoadMore,
    editableInfo,
    isLoading,
    isLoadingMore,
    hasMore,
    executionTimeMs,
    totalCount,
    rowCount,
    className,
  }: ResultsTableProps) => {
    const containerRef = useRef<HTMLDivElement>(null);

    const {
      isEditable,
      isPrimaryKeyColumn,
      isColumnEditable,
      updateCell,
      isRowModified,
      isCellModified,
      getCellDisplayValue,
      getModifiedRows,
      discardChanges,
      changesCount,
    } = useResultsTable(rows, columns, editableInfo);

    const rowVirtualizer = useVirtualizer({
      count: rows.length,
      getScrollElement: () => containerRef.current,
      estimateSize: () => ROW_HEIGHT,
      overscan: OVERSCAN,
    });

    const virtualItems = rowVirtualizer.getVirtualItems();

    const handleScroll = useCallback(() => {
      const el = containerRef.current;
      if (!el || !hasMore || isLoadingMore) return;

      const threshold = 200;
      if (el.scrollTop + el.clientHeight >= el.scrollHeight - threshold) {
        onLoadMore?.();
      }
    }, [hasMore, isLoadingMore, onLoadMore]);

    useEffect(() => {
      const el = containerRef.current;
      if (!el) return;

      el.addEventListener('scroll', handleScroll);
      return () => el.removeEventListener('scroll', handleScroll);
    }, [handleScroll]);

    if (isLoading) {
      return <LodingState className={className} />;
    }

    if (rows.length === 0) {
      return <EmptyState className={className} />;
    }

    return (
      <div
        className={cn(
          'flex flex-col h-full rounded-md border overflow-hidden',
          className,
        )}
      >
        <div className="flex-1 min-h-0">
          <div
            ref={containerRef}
            className="relative h-full w-full overflow-auto scrollbar-thin"
          >
            <div className="bg-background flex items-center border-b border-border sticky top-0 z-20">
              {columns.map(column => (
                <ColumnCell
                  key={column.name}
                  column={column}
                  isPrimaryKeyColumn={isPrimaryKeyColumn(column.name)}
                  className="w-48 min-w-48 max-w-48"
                />
              ))}
            </div>

            <div className="relative">
              {virtualItems.map(virtualRow => {
                const row = rows[virtualRow.index];
                if (!row) return null;

                const modified = isRowModified(virtualRow.index);
                const isEven = virtualRow.index % 2 === 0;

                return (
                  <ResultsTableRowCell
                    key={virtualRow.index}
                    row={row}
                    rowIndex={virtualRow.index}
                    isModified={modified}
                    isEven={isEven}
                    rowHeight={ROW_HEIGHT}
                    rowStart={virtualRow.start}
                    columns={columns}
                    getCellDisplayValue={getCellDisplayValue}
                    isCellModified={isCellModified}
                    isColumnEditable={isColumnEditable}
                    updateCell={updateCell}
                  />
                );
              })}
            </div>
          </div>
        </div>

        <DataTableStatusBar
          executionTimeMs={executionTimeMs}
          rowCount={rowCount}
          rowsLength={rows.length}
          totalCount={totalCount}
          isEditable={isEditable}
          editableInfo={editableInfo}
          changesCount={changesCount}
          isLoadingMore={isLoadingMore ?? false}
          hasMore={hasMore ?? false}
          onDiscardChanges={discardChanges}
        />
      </div>
    );
  },
);
