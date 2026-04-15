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
const INSERTED_ROW_PREFIX = 'inserted-';

const isEditableElement = (target: EventTarget | null): boolean => {
  if (!(target instanceof HTMLElement)) return false;
  const tag = target.tagName.toLowerCase();
  return (
    tag === 'input' ||
    tag === 'textarea' ||
    target.isContentEditable ||
    target.getAttribute('contenteditable') === 'true'
  );
};

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
    const [insertedRows, setInsertedRows] = useState<(string | null)[][]>([]);
    const [deletedRowsMap, setDeletedRowsMap] = useState<
      Map<string, (string | null)[]>
    >(new Map());
    const [selectedRowKey, setSelectedRowKey] = useState<string | null>(null);

    const {
      isEditable,
      isPrimaryKeyColumn,
      isColumnEditable,
      updateCell,
      isRowModified,
      isCellModified,
      getCellDisplayValue,
      discardChanges,
      getModifiedRows,
      changesCount,
    } = useResultsTable(rows, columns, editableInfo, () => {});

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
    const displayedRows = useMemo(
      () => [...rows, ...insertedRows],
      [rows, insertedRows],
    );
    const totalPendingChangesCount =
      changesCount + insertedRows.length + deletedRowsMap.size;

    const pkValuesToKey = useCallback((pkValues: (string | null)[]) => {
      return pkValues.map(value => value ?? 'NULL').join('|');
    }, []);

    const getPkValuesFromRow = useCallback(
      (row: (string | null)[]) => {
        if (!editableInfo?.primaryKeyColumnIndices?.length) return null;
        return editableInfo.primaryKeyColumnIndices.map(
          keyIndex => row[keyIndex] ?? null,
        );
      },
      [editableInfo],
    );

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

    const addRow = useCallback(() => {
      if (!isEditable || columns.length === 0) return;
      setInsertedRows(previous => [...previous, columns.map(() => null)]);
    }, [columns, isEditable]);

    const toggleSelectedRowDeletion = useCallback(() => {
      if (!selectedRowKey) return;

      if (selectedRowKey.startsWith(INSERTED_ROW_PREFIX)) {
        const insertedIndex = Number(
          selectedRowKey.replace(INSERTED_ROW_PREFIX, ''),
        );
        if (!Number.isFinite(insertedIndex)) return;
        setInsertedRows(previous =>
          previous.filter((_, index) => index !== insertedIndex),
        );
        setSelectedRowKey(null);
        return;
      }

      const rowIndex = Number(selectedRowKey);
      if (!Number.isFinite(rowIndex) || rowIndex < 0 || rowIndex >= rows.length) {
        return;
      }

      const row = rows[rowIndex];
      if (!row) return;

      const pkValues = getPkValuesFromRow(row);
      if (!pkValues) return;
      const rowKey = pkValuesToKey(pkValues);

      setDeletedRowsMap(previous => {
        const next = new Map(previous);
        if (next.has(rowKey)) {
          next.delete(rowKey);
        } else {
          next.set(rowKey, pkValues);
        }
        return next;
      });
    }, [getPkValuesFromRow, pkValuesToKey, rows, selectedRowKey]);

    const handleDiscardAll = useCallback(() => {
      discardChanges();
      setInsertedRows([]);
      setDeletedRowsMap(new Map());
      setSelectedRowKey(null);
    }, [discardChanges]);

    const handleApplyAll = useCallback(() => {
      const edits = getModifiedRows().filter(
        edit => !deletedRowsMap.has(pkValuesToKey(edit.pkValues)),
      );
      onApplyChanges({
        edits,
        insertedRows,
        deletedRowsPkValues: Array.from(deletedRowsMap.values()),
        insertColumnNames: columns.map(column => column.name),
      });
      handleDiscardAll();
    }, [
      columns,
      deletedRowsMap,
      getModifiedRows,
      handleDiscardAll,
      insertedRows,
      onApplyChanges,
      pkValuesToKey,
    ]);

    const rowVirtualizer = useVirtualizer({
      count: displayedRows.length,
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
        if (!isEditable) return;
        const key = event.key.toLowerCase();

        if (key === 'backspace' && !isEditableElement(event.target)) {
          if (selectedRowKey) {
            event.preventDefault();
            toggleSelectedRowDeletion();
          }
          return;
        }

        if (!(event.metaKey || event.ctrlKey)) return;

        if (key === 's') {
          event.preventDefault();
          if (totalPendingChangesCount > 0) {
            handleApplyAll();
          }
        }

        if (key === 'z') {
          event.preventDefault();
          if (totalPendingChangesCount > 0) {
            handleDiscardAll();
          }
        }
      };

      window.addEventListener('keydown', handleTableShortcut);
      return () => window.removeEventListener('keydown', handleTableShortcut);
    }, [
      handleApplyAll,
      handleDiscardAll,
      isEditable,
      selectedRowKey,
      tabType,
      toggleSelectedRowDeletion,
      totalPendingChangesCount,
    ]);

    if (isLoading) {
      return <LodingState className={className} />;
    }

    if (displayedRows.length === 0) {
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
                  const row = displayedRows[rowIndex];
                  if (!row) return null;
                  const isInsertedRow = rowIndex >= rows.length;
                  const insertedIndex = rowIndex - rows.length;
                  const selectedKey = isInsertedRow
                    ? `${INSERTED_ROW_PREFIX}${insertedIndex}`
                    : `${rowIndex}`;
                  const pkValues = !isInsertedRow ? getPkValuesFromRow(row) : null;
                  const rowPkKey = pkValues ? pkValuesToKey(pkValues) : null;
                  const isDeleted = !!rowPkKey && deletedRowsMap.has(rowPkKey);
                  const modified = isInsertedRow ? false : isRowModified(rowIndex);

                  return (
                    <div
                      key={`vertical-row-${rowIndex}`}
                      ref={rowVirtualizer.measureElement}
                      data-index={rowIndex}
                      className={cn(
                        'border rounded-md overflow-hidden absolute left-2 right-2 transition-colors',
                        isDeleted
                          ? 'border-red-500/50 bg-red-900/20'
                          : isInsertedRow
                            ? 'border-emerald-500/40 bg-emerald-900/10'
                            : modified
                              ? 'border-yellow-500/50'
                              : 'border-border',
                        selectedRowKey === selectedKey && 'ring-1 ring-primary/60',
                      )}
                      style={{
                        transform: `translateY(${virtualRow.start}px)`,
                        height: `${virtualRow.size}px`,
                      }}
                      onClick={() => setSelectedRowKey(selectedKey)}
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
                                  displayValue={
                                    isInsertedRow
                                      ? cell
                                      : getCellDisplayValue(
                                          rowIndex,
                                          column.name,
                                          cell,
                                        )
                                  }
                                  isModified={
                                    isInsertedRow
                                      ? false
                                      : isCellModified(rowIndex, column.name)
                                  }
                                  isEditable={
                                    isInsertedRow
                                      ? true
                                      : isColumnEditable(column.name)
                                  }
                                  onSave={newValue =>
                                    isInsertedRow
                                      ? setInsertedRows(previous =>
                                          previous.map((insertedRow, idx) =>
                                            idx === insertedIndex
                                              ? insertedRow.map(
                                                  (insertedCell, insertedCellIndex) =>
                                                    insertedCellIndex === columnIndex
                                                      ? newValue
                                                      : insertedCell,
                                                )
                                              : insertedRow,
                                          ),
                                        )
                                      : updateCell(
                                          rowIndex,
                                          column.name,
                                          cell,
                                          newValue,
                                        )
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
                    const row = displayedRows[virtualRow.index];
                    if (!row) return null;
                    const isInsertedRow = virtualRow.index >= rows.length;
                    const insertedIndex = virtualRow.index - rows.length;
                    const selectedKey = isInsertedRow
                      ? `${INSERTED_ROW_PREFIX}${insertedIndex}`
                      : `${virtualRow.index}`;
                    const pkValues = !isInsertedRow ? getPkValuesFromRow(row) : null;
                    const rowPkKey = pkValues ? pkValuesToKey(pkValues) : null;
                    const isDeleted = !!rowPkKey && deletedRowsMap.has(rowPkKey);

                    const modified = isInsertedRow
                      ? false
                      : isRowModified(virtualRow.index);
                    const isEven = virtualRow.index % 2 === 0;

                    return (
                      <ResultsTableRowCell
                        key={`row-${virtualRow.index}`}
                        row={row}
                        rowIndex={virtualRow.index}
                        isModified={modified}
                        isEven={isEven}
                        isSelected={selectedRowKey === selectedKey}
                        isDeleted={isDeleted}
                        isInserted={isInsertedRow}
                        rowHeight={ROW_HEIGHT}
                        rowStart={virtualRow.start}
                        onSelect={() => setSelectedRowKey(selectedKey)}
                        visibleColumns={visibleColumns}
                        getCellDisplayValue={(rowIndex, columnId, originalValue) => {
                          if (!isInsertedRow) {
                            return getCellDisplayValue(rowIndex, columnId, originalValue);
                          }
                          const targetColumn = columns.findIndex(
                            column => column.name === columnId,
                          );
                          if (targetColumn < 0) return originalValue;
                          return (
                            insertedRows[insertedIndex]?.[targetColumn] ?? originalValue
                          );
                        }}
                        isCellModified={(rowIndex, columnId) => {
                          if (isInsertedRow) return false;
                          return isCellModified(rowIndex, columnId);
                        }}
                        isColumnEditable={columnName =>
                          isInsertedRow ? true : isColumnEditable(columnName)
                        }
                        updateCell={(rowIndex, columnId, originalValue, newValue) => {
                          if (!isInsertedRow) {
                            updateCell(rowIndex, columnId, originalValue, newValue);
                            return;
                          }

                          const targetColumn = columns.findIndex(
                            column => column.name === columnId,
                          );
                          if (targetColumn < 0) return;

                          setInsertedRows(previous =>
                            previous.map((insertedRow, idx) =>
                              idx === insertedIndex
                                ? insertedRow.map((insertedCell, cellIndex) =>
                                    cellIndex === targetColumn
                                      ? newValue
                                      : insertedCell,
                                  )
                                : insertedRow,
                            ),
                          );
                        }}
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
          rowsLength={displayedRows.length}
          totalCount={totalCount}
          isEditable={isEditable}
          editableInfo={editableInfo}
          changesCount={changesCount}
          isLoadingMore={isLoadingMore ?? false}
          hasMore={hasMore ?? false}
          onDiscardChanges={handleDiscardAll}
          onApplyChanges={handleApplyAll}
          tabType={tabType}
          viewLayout={viewLayout}
          onViewLayoutChange={onViewLayoutChange}
          onSwitchToSql={onSwitchToSql}
          onAddRow={tabType === TabType.View && isEditable ? addRow : undefined}
          pendingInsertRowsCount={insertedRows.length}
          pendingDeleteRowsCount={deletedRowsMap.size}
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
