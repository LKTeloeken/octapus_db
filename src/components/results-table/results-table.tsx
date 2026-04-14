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
import { TabType } from '@/shared/models/tabs.types';
import { DataTableCell } from './results-table-cell/results-table-cell';

const ROW_HEIGHT = 32;
const OVERSCAN = 10;

export const ResultsTable = memo(
  ({
    columns,
    rows,
    onLoadMore,
    onApplyChanges,
    editableInfo,
    isLoading,
    isLoadingMore,
    hasMore,
    executionTimeMs,
    totalCount,
    rowCount,
    className,
    tabType,
    viewLayout = 'horizontal',
    sort,
    onSortColumn,
    onOpenForeignTable,
    onViewLayoutChange,
    onSwitchToSql,
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
      discardChanges,
      applyChanges,
      changesCount,
    } = useResultsTable(rows, columns, editableInfo, onApplyChanges);

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

    useEffect(() => {
      if (tabType !== TabType.View) return;

      const handleTableShortcut = (event: KeyboardEvent) => {
        if (!(event.metaKey || event.ctrlKey)) return;
        const key = event.key.toLowerCase();

        if (key === 's') {
          event.preventDefault();
          if (changesCount > 0) {
            applyChanges();
          }
        }

        if (key === 'z') {
          event.preventDefault();
          if (changesCount > 0) {
            discardChanges();
          }
        }
      };

      window.addEventListener('keydown', handleTableShortcut);
      return () => window.removeEventListener('keydown', handleTableShortcut);
    }, [applyChanges, changesCount, discardChanges, tabType]);

    if (isLoading) {
      return <LodingState className={className} />;
    }

    if (rows.length === 0) {
      return <EmptyState className={className} tabType={tabType} />;
    }

    const isVerticalLayout = viewLayout === 'vertical';

    return (
      <div
        className={cn(
          'flex flex-col h-full rounded-md border overflow-hidden',
          className,
        )}
      >
        <div className="flex-1 min-h-0">
          <div ref={containerRef} className="relative h-full w-full overflow-auto scrollbar-thin">
            {isVerticalLayout ? (
              <div className="p-2 space-y-2">
                {rows.map((row, rowIndex) => (
                  <div
                    key={`vertical-row-${rowIndex}`}
                    className={cn(
                      'border rounded-md overflow-hidden',
                      isRowModified(rowIndex) ? 'border-yellow-500/50' : 'border-border',
                    )}
                  >
                    <div className="bg-muted/40 px-2 py-1 text-xs text-muted-foreground">
                      Row {rowIndex + 1}
                    </div>
                    <div>
                      {columns.map((column, cellIndex) => {
                        const cell = row[cellIndex];
                        return (
                          <div
                            key={`vertical-cell-${rowIndex}-${column.name}`}
                            className="grid grid-cols-[180px_1fr] border-t border-border"
                          >
                            <div className="px-2 py-1.5 text-xs bg-muted/20 border-r border-border truncate">
                              {column.name}
                            </div>
                            <div className="min-w-0">
                              <DataTableCell
                                value={cell}
                                columnType={column.typeName}
                                displayValue={getCellDisplayValue(rowIndex, column.name, cell)}
                                isModified={isCellModified(rowIndex, column.name)}
                                isEditable={isColumnEditable(column.name)}
                                onSave={newValue =>
                                  updateCell(rowIndex, column.name, cell, newValue)
                                }
                              />
                            </div>
                          </div>
                        );
                      })}
                    </div>
                  </div>
                ))}
              </div>
            ) : (
              <div>
                <div className="bg-background flex items-center border-b border-border sticky top-0 z-20">
                  {columns.map(column => (
                    <ColumnCell
                      key={`column-${column.name}`}
                      column={column}
                      isPrimaryKeyColumn={isPrimaryKeyColumn(column.name)}
                      sortable={tabType === 'view'}
                      sort={sort}
                      onSortColumn={onSortColumn}
                      onOpenForeignTable={onOpenForeignTable}
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
                        key={`row-${virtualRow.index}`}
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
            )}
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
          onApplyChanges={applyChanges}
          tabType={tabType}
          viewLayout={viewLayout}
          onViewLayoutChange={onViewLayoutChange}
          onSwitchToSql={onSwitchToSql}
        />
      </div>
    );
  },
);
