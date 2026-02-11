import * as React from 'react';
import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  memo,
} from 'react';
import useVirtualization from '@/shared/hooks/use-virtualization/use-virtualization';
import { cn } from '@/lib/utils';
import { Spinner } from '@/components/ui/spinner';

import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table';

import type { ResultsTableProps } from './results-table.types';
import { useStyles } from './results-table.styles';

export const ResultsTable = memo(function ResultsTable({
  columns,
  rows,
  rowHeight = 36,
  overscan = 8,
  className,
  emptyMessage = 'No results found.',
  hasMore = false,
  isLoadingMore = false,
  onLoadMore,
}: ResultsTableProps) {
  const styles = useStyles();

  const { parentRef, rowVirtualizer } = useVirtualization(
    rows.length,
    rowHeight,
    overscan,
  );

  const virtualItems = rowVirtualizer.getVirtualItems();

  // Refs and state for column alignment
  const headerRefs = useRef<(HTMLTableCellElement | null)[]>([]);
  const [colWidths, setColWidths] = useState<number[]>([]);

  // Measure widths after header render (includes resize and column changes)
  useLayoutEffect(() => {
    if (!columns.length) return;
    const widths = headerRefs.current.map(
      el => el?.getBoundingClientRect().width || 0,
    );
    // Avoid unnecessary re-render if nothing changed
    if (widths.length && widths.some((w, i) => w !== colWidths[i])) {
      setColWidths(widths);
    }
  }, [columns, rows.length]);

  // Recalculate on window resize
  useLayoutEffect(() => {
    const handle = () => {
      const widths = headerRefs.current.map(
        el => el?.getBoundingClientRect().width || 0,
      );
      setColWidths(widths);
    };
    window.addEventListener('resize', handle);
    return () => window.removeEventListener('resize', handle);
  }, []);

  // Infinite scroll detection
  const handleScroll = useCallback(() => {
    const el = parentRef.current;
    if (!el || !hasMore || isLoadingMore) return;

    const threshold = 200;
    if (el.scrollTop + el.clientHeight >= el.scrollHeight - threshold) {
      onLoadMore?.();
    }
  }, [hasMore, isLoadingMore, onLoadMore]);

  useEffect(() => {
    const el = parentRef.current;
    if (!el) return;

    el.addEventListener('scroll', handleScroll);
    return () => el.removeEventListener('scroll', handleScroll);
  }, [handleScroll]);

  if (rows.length === 0) {
    return (
      <div className={cn(styles.emptyContainer, className)}>
        <p className={styles.emptyText}>{emptyMessage}</p>
      </div>
    );
  }

  return (
    <Table
      ref={parentRef as React.RefObject<HTMLDivElement>}
      containerClassName={cn(styles.tableContainer, className)}
    >
      {/* Sticky header */}
      <TableHeader className={styles.header}>
        <TableRow>
          {columns.map((col, idx) => (
            <TableHead
              key={col.name}
              ref={el => {
                headerRefs.current[idx] = el;
              }}
              className={styles.headerCell}
              style={colWidths[idx] ? { width: colWidths[idx] } : undefined}
            >
              {col.name}
            </TableHead>
          ))}
        </TableRow>
      </TableHeader>

      <TableBody
        style={{
          height: `${rowVirtualizer.getTotalSize()}px`,
          position: 'relative',
        }}
      >
        {virtualItems.map(virtualItem => {
          const row = rows[virtualItem.index];
          if (!row) return null;

          const isEven = virtualItem.index % 2 === 0;

          return (
            <TableRow
              key={virtualItem.key}
              data-index={virtualItem.index}
              className={isEven ? styles.rowEven : styles.rowOdd}
              style={{
                position: 'absolute',
                top: 0,
                left: 0,
                width: '100%',
                height: `${rowHeight}px`,
                transform: `translateY(${virtualItem.start}px)`,
              }}
            >
              {columns.map((col, cIdx) => {
                const width = colWidths[cIdx];
                return (
                  <TableCell
                    key={`${col.name}-${virtualItem.index}`}
                    style={
                      width
                        ? { width, minWidth: width, maxWidth: width }
                        : undefined
                    }
                  >
                    <span className={styles.cellText}>
                      {String(row[cIdx] || '')}
                    </span>
                  </TableCell>
                );
              })}
            </TableRow>
          );
        })}
      </TableBody>

      {isLoadingMore && (
        <tfoot>
          <tr>
            <td colSpan={columns.length}>
              <div className={styles.loadingMoreContainer}>
                <Spinner className={styles.loadingMoreSpinner} />
                <span className={styles.loadingMoreText}>
                  Carregando mais resultados...
                </span>
              </div>
            </td>
          </tr>
        </tfoot>
      )}
    </Table>
  );
});
