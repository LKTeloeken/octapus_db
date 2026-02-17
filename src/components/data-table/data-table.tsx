import { memo, useCallback, useEffect, useMemo, useRef } from 'react';
import {
  useReactTable,
  getCoreRowModel,
  flexRender,
  type ColumnDef,
  type Header,
} from '@tanstack/react-table';
import { useVirtualizer } from '@tanstack/react-virtual';
import { cn } from '@/lib/utils';
import { Spinner } from '@/components/ui/spinner';
import { Typography } from '@/components/ui/typography';

import type { DataTableProps, DataTableRow } from './data-table.types';
import { useDataTable } from './use-data-table';
import { DataTableCell } from './data-table-cell';
import { useStyles } from './data-table.styles';

const ROW_HEIGHT = 32;
const OVERSCAN = 10;

export const DataTable = memo(function DataTable({
  columns: queryColumns,
  rows,
  editableInfo,
  isLoading = false,
  isLoadingMore = false,
  hasMore = false,
  executionTimeMs,
  totalCount,
  rowCount,
  onLoadMore,
  className,
}: DataTableProps) {
  const styles = useStyles();
  const containerRef = useRef<HTMLDivElement>(null);

  const {
    isEditable,
    changesCount,
    isColumnEditable,
    updateCell,
    isRowModified,
    isCellModified,
    getCellDisplayValue,
    discardChanges,
  } = useDataTable(rows, queryColumns, editableInfo);

  // Column index lookup: column name -> index in the row array
  const columnIndexMap = useMemo(
    () => new Map(queryColumns.map((col, idx) => [col.name, idx])),
    [queryColumns],
  );

  // Column defs for TanStack Table (used for headers and column model)
  const columnDefs = useMemo<ColumnDef<DataTableRow>[]>(
    () =>
      queryColumns.map((col, idx) => ({
        id: col.name,
        header: `${col.name}-${col.typeName}`,
        accessorFn: (row: DataTableRow) => row[idx],
        size: 180,
        minSize: 80,
        maxSize: 400,
      })),
    [queryColumns],
  );

  const table = useReactTable({
    data: rows,
    columns: columnDefs,
    getCoreRowModel: getCoreRowModel(),
  });

  // Total width of all columns for proper horizontal layout
  const totalWidth = useMemo(() => table.getTotalSize(), [table]);

  const { rows: tableRows } = table.getRowModel();

  const rowVirtualizer = useVirtualizer({
    count: tableRows.length,
    getScrollElement: () => containerRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: OVERSCAN,
  });

  const virtualItems = rowVirtualizer.getVirtualItems();

  // Infinite scroll detection
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

  const renderHeaderLabel = useCallback(
    (header: Header<DataTableRow, unknown>) => {
      const [name, type] =
        header?.column?.columnDef?.header?.toString().split('-') ?? [];

      if (header?.isPlaceholder) return null;

      return flexRender(
        <div className="flex flex-col items-start gap-1">
          <div>{name}</div>
          <div className="text-muted-foreground text-xs">({type})</div>
        </div>,
        header.getContext(),
      );
    },
    [],
  );

  // Loading state
  if (isLoading) {
    return (
      <div className={cn(styles.root, className)}>
        <div className={styles.loadingContainer}>
          <div className={styles.loadingContent}>
            <Spinner className={styles.loadingSpinner} />
            <Typography variant="p" className={styles.loadingText}>
              Executando query...
            </Typography>
          </div>
        </div>
      </div>
    );
  }

  // Empty state
  if (!queryColumns.length || !rows.length) {
    return (
      <div className={cn(styles.root, className)}>
        <div className={styles.emptyContainer}>
          <Typography variant="p" className={styles.emptyText}>
            Execute uma query para ver resultados
          </Typography>
        </div>
      </div>
    );
  }

  const hasResult = rows.length > 0 || rowCount !== undefined;

  console.log('totalWidth', totalWidth);
  console.log('rowVirtualizer.getTotalSize()', rowVirtualizer.getTotalSize());

  return (
    <div className={cn(styles.root, className)}>
      <div className={styles.tableWrapper}>
        <div ref={containerRef} className={styles.container}>
          <div className={styles.table} style={{ minWidth: totalWidth }}>
            {table.getHeaderGroups().map(headerGroup => (
              <div
                key={headerGroup.id}
                className={
                  'bg-background flex items-center border-b border-border sticky top-0 z-20'
                }
                style={{ width: totalWidth }}
              >
                {headerGroup.headers.map(header => (
                  <div
                    key={header.id}
                    className={
                      'bg-background shadow-[0_1px_0_0_var(--color-border)] h-9 px-2 text-left align-middle font-medium text-foreground whitespace-nowrap text-xs select-none border-r border-border flex items-center justify-start'
                    }
                    style={{
                      width: header.getSize(),
                      minWidth: header.getSize(),
                    }}
                  >
                    {renderHeaderLabel(header)}
                  </div>
                ))}
              </div>
            ))}

            <div
              style={{
                minHeight: `${rowVirtualizer.getTotalSize()}px`,
                position: 'relative',
              }}
            >
              {virtualItems.map(virtualRow => {
                const row = tableRows[virtualRow.index];
                if (!row) return null;

                const modified = isRowModified(virtualRow.index);
                const isEven = virtualRow.index % 2 === 0;

                return (
                  <tr
                    key={row.id}
                    data-index={virtualRow.index}
                    className={cn(
                      styles.bodyRow,
                      modified
                        ? styles.bodyRowModified
                        : isEven
                          ? styles.bodyRowEven
                          : '',
                    )}
                    style={{
                      position: 'absolute',
                      top: 0,
                      left: 0,
                      width: totalWidth,
                      height: `${ROW_HEIGHT}px`,
                      transform: `translateY(${virtualRow.start}px)`,
                    }}
                  >
                    {row.getVisibleCells().map(cell => {
                      const colIdx = columnIndexMap.get(cell.column.id) ?? 0;
                      const originalValue =
                        rows[virtualRow.index]?.[colIdx] ?? null;
                      const columnId = cell.column.id;
                      const displayValue = getCellDisplayValue(
                        virtualRow.index,
                        columnId,
                        originalValue,
                      );
                      const editable =
                        isEditable &&
                        isColumnEditable(columnId) &&
                        originalValue !== null;
                      const cellModified = isCellModified(
                        virtualRow.index,
                        columnId,
                      );

                      const colTypeName =
                        queryColumns[colIdx]?.typeName ?? 'text';

                      return (
                        <td
                          key={cell.id}
                          className={styles.bodyCell}
                          style={{
                            width: cell.column.getSize(),
                            minWidth: cell.column.getSize(),
                            maxWidth: cell.column.getSize(),
                          }}
                        >
                          <DataTableCell
                            value={originalValue}
                            displayValue={displayValue}
                            isEditable={editable}
                            isModified={cellModified}
                            columnType={colTypeName}
                            onSave={newValue =>
                              updateCell(
                                virtualRow.index,
                                columnId,
                                originalValue,
                                newValue,
                              )
                            }
                          />
                        </td>
                      );
                    })}
                  </tr>
                );
              })}
            </div>
          </div>

          {isLoadingMore && (
            <div className={styles.loadingMoreContainer}>
              <Spinner className={styles.loadingMoreSpinner} />
              <span className={styles.loadingMoreText}>
                Carregando mais resultados...
              </span>
            </div>
          )}
        </div>
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
            {isEditable && editableInfo && (
              <span className="text-green-400">
                {editableInfo.schema}.{editableInfo.table}
              </span>
            )}
          </div>
          <div className={styles.statusBarRight}>
            {changesCount > 0 && (
              <>
                <span className={styles.statusBarChanges}>
                  {changesCount} alteraç{changesCount === 1 ? 'ão' : 'ões'}{' '}
                  pendente{changesCount === 1 ? '' : 's'}
                </span>
                <button
                  type="button"
                  className="px-2 py-0.5 text-xs rounded border border-border hover:bg-muted/60 transition-colors"
                  onClick={discardChanges}
                >
                  Descartar
                </button>
              </>
            )}
            {isLoadingMore && (
              <>
                <Spinner className={styles.statusBarSpinner} />
                <span>Carregando...</span>
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
});
