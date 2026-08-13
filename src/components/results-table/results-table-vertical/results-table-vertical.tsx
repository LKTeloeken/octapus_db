import { useVirtualizer } from '@tanstack/react-virtual';
import { memo, useCallback, useEffect, useRef } from 'react';
import { cn } from '@/lib/utils';
import { DataTableCell } from '../results-table-cell/results-table-cell';
import type { ResultsTableVerticalProps } from './results-table-vertical.types';

const ROW_HEIGHT = 32;
const HEADER_HEIGHT = 28;
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
    onActivateCell,
    onCloseCell,
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
          className="sticky top-0 z-20 bg-background border-b border-border"
          style={{ width: `${contentWidth}px`, height: `${HEADER_HEIGHT}px` }}
        >
          <div
            className="sticky left-0 z-30 bg-background border-r border-border px-2 py-1.5 text-xs font-medium text-muted-foreground"
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
                  'absolute top-0 border-r border-border px-2 py-1.5 text-xs font-mono text-left cursor-pointer outline-none',
                  removed
                    ? 'bg-red-900/30 text-red-300'
                    : added
                      ? 'bg-green-900/30 text-green-300'
                      : isRowModified(virtualRecord.index)
                        ? 'text-yellow-300'
                        : 'text-muted-foreground',
                  selected && 'ring-1 ring-inset ring-primary',
                )}
                style={{
                  left: `${LABEL_WIDTH + virtualRecord.start}px`,
                  width: `${virtualRecord.size}px`,
                  height: `${HEADER_HEIGHT}px`,
                }}
              >
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

            const isEven = virtualField.index % 2 === 0;
            const isPk = isPrimaryKeyColumn(column.name);
            const columnSelected = isColumnSelected(column.name);

            return (
              <div
                key={virtualField.key}
                className={cn(
                  'absolute top-0 left-0',
                  isEven ? 'bg-muted' : 'bg-transparent',
                )}
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
                    'sticky left-0 z-10 bg-background border-r border-border flex flex-col justify-center px-2 text-left cursor-pointer outline-none w-full',
                    columnSelected && 'bg-accent/40',
                  )}
                  style={{ width: `${LABEL_WIDTH}px`, height: `${ROW_HEIGHT}px` }}
                >
                  <div className="text-xs truncate">{column.name}</div>
                  <div className="text-[10px] text-muted-foreground truncate">
                    {column.typeName}
                    {isPk ? ' (PK)' : ''}
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
                      className="absolute top-0 border-r border-border"
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
