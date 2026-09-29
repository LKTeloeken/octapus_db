import { useVirtualizer } from '@tanstack/react-virtual';
import { memo, useCallback, useEffect, useRef } from 'react';
import { cn } from '@/lib/utils';
import { DataTableCell } from '../results-table-cell/results-table-cell';
import type { ResultsTableVerticalProps } from './results-table-vertical.types';

const ROW_HEIGHT = 32;
const HEADER_HEIGHT = 32;
const LABEL_WIDTH = 208;
const RECORD_WIDTH = 192;
const OVERSCAN_Y = 10;
const OVERSCAN_X = 3;

// Fields → rows, records → columns. Both axes are virtualized; the field-name
// column and the header row stay pinned while records scroll horizontally.
export const ResultsTableVertical = memo(
  ({
    columns,
    columnIndices,
    rows,
    isPrimaryKeyColumn,
    isColumnEditable,
    isCellModified,
    isRowModified,
    isRowAdded,
    isRowRemoved,
    isRowSelected,
    isColumnSelected,
    getCellDisplayValue,
    updateCell,
    activeCell,
    focusedCell,
    onActivateCell,
    onCloseCell,
    onFocusCell,
    onSelectRow,
    onSelectColumn,
    hasMore,
    isLoadingMore,
    onLoadMore,
  }: ResultsTableVerticalProps) => {
    const containerRef = useRef<HTMLDivElement>(null);

    // Field axis (one row per column).
    const fieldVirtualizer = useVirtualizer({
      count: columns.length,
      getScrollElement: () => containerRef.current,
      estimateSize: () => ROW_HEIGHT,
      overscan: OVERSCAN_Y,
    });

    // Record axis (one column per row).
    const recordVirtualizer = useVirtualizer({
      horizontal: true,
      count: rows.length,
      getScrollElement: () => containerRef.current,
      estimateSize: () => RECORD_WIDTH,
      overscan: OVERSCAN_X,
    });

    const virtualFields = fieldVirtualizer.getVirtualItems();
    const virtualRecords = recordVirtualizer.getVirtualItems();
    const contentWidth = LABEL_WIDTH + recordVirtualizer.getTotalSize();

    // Records grow to the right — load more near the right edge.
    const handleScroll = useCallback(() => {
      const el = containerRef.current;
      if (!el || !hasMore || isLoadingMore) return;

      const threshold = 300;
      if (el.scrollLeft + el.clientWidth >= el.scrollWidth - threshold) {
        onLoadMore?.();
      }
    }, [hasMore, isLoadingMore, onLoadMore]);

    useEffect(() => {
      const el = containerRef.current;
      if (!el) return;

      el.addEventListener('scroll', handleScroll);
      return () => el.removeEventListener('scroll', handleScroll);
    }, [handleScroll]);

    return (
      <div
        ref={containerRef}
        className="relative h-full w-full overflow-auto scrollbar-thin"
      >
        {/* Header: pinned corner + one label per visible record */}
        <div
          className="sticky top-0 z-20 border-b border-line bg-surface-2"
          style={{ width: `${contentWidth}px`, height: `${HEADER_HEIGHT}px` }}
        >
          <div
            className="sticky left-0 z-30 flex items-center border-r border-line-subtle bg-surface-2 px-2.5 text-micro font-medium uppercase text-fg-subtle"
            style={{ width: `${LABEL_WIDTH}px`, height: `${HEADER_HEIGHT}px` }}
          >
            Coluna
          </div>
          {virtualRecords.map(virtualRecord => {
            const added = isRowAdded(virtualRecord.index);
            const removed = isRowRemoved(virtualRecord.index);
            const selected = isRowSelected(virtualRecord.index);

            return (
              <button
                type="button"
                key={virtualRecord.key}
                onClick={event => onSelectRow(virtualRecord.index, event)}
                className={cn(
                  'absolute top-0 cursor-pointer border-r border-line-subtle px-2.5 text-left font-mono text-[11px] tabular-nums outline-none focus-visible:ring-1 focus-visible:ring-inset focus-visible:ring-ring',
                  // Mesma codificação da grade: estado pendente + seleção em Iris.
                  selected
                    ? 'bg-iris-soft text-iris-text'
                    : removed
                      ? 'bg-danger-soft text-danger line-through'
                      : added
                        ? 'bg-success-soft text-success'
                        : isRowModified(virtualRecord.index)
                          ? 'text-warning'
                          : 'text-fg-subtle',
                )}
                style={{
                  left: `${LABEL_WIDTH + virtualRecord.start}px`,
                  width: `${virtualRecord.size}px`,
                  height: `${HEADER_HEIGHT}px`,
                }}
              >
                {removed ? '− ' : added ? '+ ' : isRowModified(virtualRecord.index) ? '• ' : ''}
                #{virtualRecord.index + 1}
              </button>
            );
          })}
        </div>

        {/* Body: one virtualized row per field, virtualized record cells */}
        <div
          className="relative"
          style={{
            width: `${contentWidth}px`,
            height: `${fieldVirtualizer.getTotalSize()}px`,
          }}
        >
          {virtualFields.map(virtualField => {
            const column = columns[virtualField.index];
            if (!column) return null;

            const isPk = isPrimaryKeyColumn(column.name);
            const columnSelected = isColumnSelected(column.name);

            return (
              <div
                key={virtualField.key}
                className="absolute top-0 left-0"
                style={{
                  height: `${ROW_HEIGHT}px`,
                  width: `${contentWidth}px`,
                  transform: `translateY(${virtualField.start}px)`,
                }}
              >
                <button
                  type="button"
                  onClick={() => onSelectColumn(column.name)}
                  className={cn(
                    'sticky left-0 z-10 flex w-full cursor-pointer flex-col justify-center border-r border-b border-line-subtle bg-surface-1 px-2.5 text-left outline-none',
                    columnSelected && 'bg-iris-soft',
                  )}
                  style={{ width: `${LABEL_WIDTH}px`, height: `${ROW_HEIGHT}px` }}
                >
                  <div className="truncate text-small font-medium text-fg">
                    {column.name}
                  </div>
                  <div className="truncate font-mono text-[11px] leading-[14px] text-fg-subtle">
                    {column.typeName}
                    {isPk ? ' · PK' : ''}
                  </div>
                </button>

                {virtualRecords.map(virtualRecord => {
                  const cell =
                    rows[virtualRecord.index]?.[columnIndices[virtualField.index]];
                  const displayValue = getCellDisplayValue(
                    virtualRecord.index,
                    column.name,
                    cell ?? null,
                  );
                  // Added records are fully editable; deleted ones are frozen.
                  const editable = isRowRemoved(virtualRecord.index)
                    ? false
                    : isRowAdded(virtualRecord.index) ||
                      isColumnEditable(column.name);

                  return (
                    <div
                      key={virtualRecord.key}
                      onMouseDown={() =>
                        onFocusCell?.(virtualRecord.index, column.name)
                      }
                      className={cn(
                        'absolute top-0 border-r border-b border-line-subtle',
                        isRowRemoved(virtualRecord.index)
                          ? 'bg-danger-soft'
                          : isRowAdded(virtualRecord.index) && 'bg-success-soft',
                        focusedCell?.rowIndex === virtualRecord.index &&
                          focusedCell.columnName === column.name &&
                          'z-10 ring-[1.5px] ring-inset ring-ring',
                      )}
                      style={{
                        left: `${LABEL_WIDTH + virtualRecord.start}px`,
                        width: `${virtualRecord.size}px`,
                        height: `${ROW_HEIGHT}px`,
                      }}
                    >
                      <DataTableCell
                        value={cell ?? null}
                        displayValue={displayValue}
                        columnType={column.typeName}
                        isModified={isCellModified(
                          virtualRecord.index,
                          column.name,
                        )}
                        isRemoved={isRowRemoved(virtualRecord.index)}
                        isEditable={editable}
                        rowIndex={virtualRecord.index}
                        columnName={column.name}
                        isActive={
                          activeCell?.rowIndex === virtualRecord.index &&
                          activeCell.columnName === column.name
                        }
                        onActivate={onActivateCell}
                        onClose={onCloseCell}
                        updateCell={updateCell}
                      />
                    </div>
                  );
                })}
              </div>
            );
          })}
        </div>
      </div>
    );
  },
);

ResultsTableVertical.displayName = 'ResultsTableVertical';
