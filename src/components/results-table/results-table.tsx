import { cn } from '@/lib/utils';
import type { ResultsTableProps } from './results-table.types';
import { memo, useCallback, useEffect, useMemo, useRef, useState } from 'react';
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
const VERTICAL_BASE_ROW_HEIGHT = 44;
const VERTICAL_HEADER_HEIGHT = 28;

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
    const [hiddenColumns, setHiddenColumns] = useState<Set<string>>(new Set());

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

    const visibleColumns = useMemo(
      () =>
        columns
          .map((column, columnIndex) => ({ column, columnIndex }))
          .filter(({ column }) => !hiddenColumns.has(column.name)),
      [columns, hiddenColumns],
    );

    const visibleColumnNames = useMemo(
      () => visibleColumns.map(({ column }) => column.name),
      [visibleColumns],
    );

    const visibleColumnCount = visibleColumns.length;

    const toggleColumnVisibility = useCallback((columnName: string) => {
      setHiddenColumns(previous => {
        const next = new Set(previous);
        if (next.has(columnName)) {
          next.delete(columnName);
        } else if (next.size < columns.length - 1) {
          next.add(columnName);
        }
        return next;
      });
    }, [columns.length]);

    const showAllColumns = useCallback(() => {
      setHiddenColumns(new Set());
    }, []);

    useEffect(() => {
      setHiddenColumns(previous => {
        if (previous.size === 0) return previous;
        const currentNames = new Set(columns.map(column => column.name));
        const next = new Set(
          Array.from(previous).filter(columnName => currentNames.has(columnName)),
        );
        return next.size === previous.size ? previous : next;
      });
    }, [columns]);

    const rowVirtualizer = useVirtualizer({
      count: rows.length,
      getScrollElement: () => containerRef.current,
      estimateSize: () =>
        viewLayout === 'vertical'
          ? VERTICAL_HEADER_HEIGHT + visibleColumnCount * VERTICAL_BASE_ROW_HEIGHT
          : ROW_HEIGHT,
      overscan: OVERSCAN,
      measureElement: element => element.getBoundingClientRect().height,
    });

    const virtualItems = rowVirtualizer.getVirtualItems();

    useEffect(() => {
      rowVirtualizer.measure();
    }, [rowVirtualizer, viewLayout, visibleColumnCount]);

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
              <div
                className="relative"
                style={{ height: `${rowVirtualizer.getTotalSize()}px` }}
              >
                {virtualItems.map(virtualRow => {
                  const rowIndex = virtualRow.index;
                  const row = rows[rowIndex];
                  if (!row) return null;

                  return (
                    <div
                      key={`vertical-row-${rowIndex}`}
                      ref={rowVirtualizer.measureElement}
                      className={cn(
                        'border rounded-md overflow-hidden absolute left-2 right-2',
                        isRowModified(rowIndex) ? 'border-yellow-500/50' : 'border-border',
                      )}
                      style={{
                        transform: `translateY(${virtualRow.start}px)`,
                        height: `${virtualRow.size}px`,
                      }}
                    >
                      <div className="bg-muted/40 px-2 py-1 text-xs text-muted-foreground">
                        Row {rowIndex + 1}
                      </div>
                        <div>
                        {visibleColumns.map(({ column, columnIndex }) => {
                          const cell = row[columnIndex] ?? null;
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
                  );
                })}
              </div>
            ) : (
              <div>
                  <div className="bg-background flex items-center border-b border-border sticky top-0 z-20">
                  {visibleColumns.map(({ column }) => (
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
                        visibleColumns={visibleColumns}
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
          columns={tabType === TabType.View ? columns : undefined}
          visibleColumnNames={
            tabType === TabType.View ? visibleColumnNames : undefined
          }
          onToggleColumnVisibility={
            tabType === TabType.View ? toggleColumnVisibility : undefined
          }
          onShowAllColumns={tabType === TabType.View ? showAllColumns : undefined}
        />
      </div>
    );
  },
);
