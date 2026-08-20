import { cn } from '@/lib/utils';
import type {
  ResultsTableProps,
  ResultsViewMode,
  RowSelectionMode,
} from './results-table.types';
import { memo, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { QueryColumnInfo } from '@/api/types/query.types';
import ColumnCell from './results-table-column-cell/results-table-column-cell';
import {
  nextBooleanValue,
  resolveCellEditor,
} from './results-table-cell/use-resolve-cell-editor';
import useResultsTable from './use-results-table';
import { useResultsTableKeyboard } from './use-results-table-keyboard';
import { useVirtualizer } from '@tanstack/react-virtual';
import { DataTableStatusBar } from './results-table-status-bar/results-table-status-bar';
import { EmptyState } from './empty-state';
import { LoadingState } from './loading-state';
import { ResultsTableRowCell } from './results-table-row-cell/results-table-row-cell';
import { ResultsTableVertical } from './results-table-vertical/results-table-vertical';

const ROW_HEIGHT = 32;
const HEADER_HEIGHT = 36;
const COLUMN_WIDTH = 192;
const MIN_COLUMN_WIDTH = 60;
const GUTTER_WIDTH = 48;
const OVERSCAN_Y = 10;
const OVERSCAN_X = 3;

/** Modo de seleção de um clique no corpo da linha, ou `null` quando o clique é
 *  simples — nesse caso ele só move o cursor de célula, sem mexer na seleção. */
const selectionModeFromEvent = (
  event: React.MouseEvent,
): RowSelectionMode | null => {
  if (event.shiftKey) return 'range';
  if (event.metaKey || event.ctrlKey) return 'toggle';
  return null;
};

/** Resolve the selection mode for a click on the index gutter — a plain click
 *  adds to the selection (toggle); Shift extends the range from the anchor. */
const selectionModeFromGutterEvent = (
  event: React.MouseEvent,
): RowSelectionMode => {
  if (event.shiftKey) return 'range';
  return 'toggle';
};

export const ResultsTable = memo(
  ({
    columns,
    rows,
    onSort,
    onLoadMore,
    onSave,
    editableInfo,
    hiddenColumns,
    activeSort,
    isLoading,
    isLoadingMore,
    hasMore,
    executionTimeMs,
    totalCount,
    rowCount,
    emptyMessage,
    className,
  }: ResultsTableProps) => {
    const containerRef = useRef<HTMLDivElement>(null);
    const [viewMode, setViewMode] = useState<ResultsViewMode>('table');

    // Larguras ajustadas manualmente (arraste no header), por nome de coluna —
    // sobrevive à ocultação/reexibição, que desloca os índices visíveis.
    const [columnWidths, setColumnWidths] = useState<Record<string, number>>(
      {},
    );
    const resizeDragRef = useRef<{
      columnName: string;
      startX: number;
      startWidth: number;
      currentWidth: number;
    } | null>(null);

    const {
      isEditable,
      isPrimaryKeyColumn,
      isColumnEditable,
      displayRows,
      updateCell,
      isRowModified,
      isCellModified,
      isRowAdded,
      isRowRemoved,
      isRowSelected,
      isColumnSelected,
      getCellDisplayValue,
      activeCell,
      activateCell,
      deactivateCell,
      focusedCell,
      focusCell,
      setFocusedCell,
      toggleRowSelection,
      toggleColumnSelection,
      addRow,
      discardChanges,
      save,
      changesCount,
      addedCount,
      removedCount,
      pendingCount,
    } = useResultsTable(rows, columns, editableInfo, onSave);

    // Colunas ocultas afetam só a renderização: filtramos quais colunas desenhar
    // e guardamos o índice original de cada uma para ler `row[índiceOriginal]`.
    // `useResultsTable` continua operando sobre o array `columns` completo.
    const { visibleColumns, visibleColumnIndices } = useMemo(() => {
      if (!hiddenColumns || hiddenColumns.size === 0) {
        return {
          visibleColumns: columns,
          visibleColumnIndices: columns.map((_, i) => i),
        };
      }
      const cols: QueryColumnInfo[] = [];
      const indices: number[] = [];
      columns.forEach((column, index) => {
        if (!hiddenColumns.has(column.name)) {
          cols.push(column);
          indices.push(index);
        }
      });
      return { visibleColumns: cols, visibleColumnIndices: indices };
    }, [columns, hiddenColumns]);

    // Chave estável por nome (não por índice): preserva a largura de cada
    // coluna quando outra é ocultada/reexibida e os índices se deslocam.
    const getColumnKey = useCallback(
      (index: number) => visibleColumns[index]?.name ?? index,
      [visibleColumns],
    );

    const estimateColumnSize = useCallback(
      (index: number) =>
        columnWidths[visibleColumns[index]?.name ?? ''] ?? COLUMN_WIDTH,
      [columnWidths, visibleColumns],
    );

    const rowVirtualizer = useVirtualizer({
      count: displayRows.length,
      getScrollElement: () => containerRef.current,
      estimateSize: () => ROW_HEIGHT,
      overscan: OVERSCAN_Y,
    });

    const columnVirtualizer = useVirtualizer({
      horizontal: true,
      count: visibleColumns.length,
      getScrollElement: () => containerRef.current,
      estimateSize: estimateColumnSize,
      getItemKey: getColumnKey,
      overscan: OVERSCAN_X,
    });

    const virtualRows = rowVirtualizer.getVirtualItems();
    const virtualColumns = columnVirtualizer.getVirtualItems();
    const columnsWidth = columnVirtualizer.getTotalSize();
    const contentWidth = GUTTER_WIDTH + columnsWidth;

    // Arraste na borda direita do header: redimensiona ao vivo via
    // `resizeItem` (recalcula a virtualização na hora) e só grava em estado
    // (por nome) ao soltar, para sobreviver a um próximo resize do array.
    const handleResizeMove = useCallback(
      (event: MouseEvent) => {
        const drag = resizeDragRef.current;
        if (!drag) return;
        const index = visibleColumns.findIndex(c => c.name === drag.columnName);
        if (index === -1) return;
        const nextWidth = Math.max(
          MIN_COLUMN_WIDTH,
          drag.startWidth + (event.clientX - drag.startX),
        );
        drag.currentWidth = nextWidth;
        columnVirtualizer.resizeItem(index, nextWidth);
      },
      [visibleColumns, columnVirtualizer],
    );

    const handleResizeEnd = useCallback(() => {
      const drag = resizeDragRef.current;
      resizeDragRef.current = null;
      document.body.style.cursor = '';
      document.body.style.userSelect = '';
      document.removeEventListener('mousemove', handleResizeMove);
      document.removeEventListener('mouseup', handleResizeEnd);
      if (!drag) return;
      setColumnWidths(prev => ({
        ...prev,
        [drag.columnName]: drag.currentWidth,
      }));
    }, [handleResizeMove]);

    const handleResizeStart = useCallback(
      (event: React.MouseEvent, columnName: string, width: number) => {
        event.preventDefault();
        event.stopPropagation();
        resizeDragRef.current = {
          columnName,
          startX: event.clientX,
          startWidth: width,
          currentWidth: width,
        };
        document.body.style.cursor = 'col-resize';
        document.body.style.userSelect = 'none';
        document.addEventListener('mousemove', handleResizeMove);
        document.addEventListener('mouseup', handleResizeEnd);
      },
      [handleResizeMove, handleResizeEnd],
    );

    // Evita listener/estilo pendurado se a aba fechar com o arraste em curso.
    useEffect(() => {
      return () => {
        document.removeEventListener('mousemove', handleResizeMove);
        document.removeEventListener('mouseup', handleResizeEnd);
        document.body.style.cursor = '';
        document.body.style.userSelect = '';
      };
    }, [handleResizeMove, handleResizeEnd]);

    // Click on the row body: só com modificador (Shift estende, Cmd/Ctrl
    // alterna). Um clique simples é do cursor de célula, tratado abaixo.
    const handleSelectRowBody = useCallback(
      (rowIndex: number, event: React.MouseEvent) => {
        const mode = selectionModeFromEvent(event);
        if (mode) toggleRowSelection(rowIndex, mode);
      },
      [toggleRowSelection],
    );

    // Clique numa célula: move o cursor e traz o foco do DOM de volta para o
    // container, o único tab stop da grade.
    const handleFocusCell = useCallback(
      (rowIndex: number, columnName: string) => {
        focusCell(rowIndex, columnName);
        containerRef.current?.focus();
      },
      [focusCell],
    );

    // Enter na célula sob o cursor. Booleano é checkbox inline, não abre
    // Popover: ativá-lo deixaria `activeCell` preso e mataria as setas — então
    // o Enter alterna o valor direto, como o clique no checkbox faria.
    const handleActivateCell = useCallback(
      (rowIndex: number, columnName: string) => {
        const columnIndex = visibleColumns.findIndex(
          c => c.name === columnName,
        );
        const column = visibleColumns[columnIndex];
        if (!column) return;

        if (resolveCellEditor(column.typeName) === 'boolean') {
          const editable = isRowRemoved(rowIndex)
            ? false
            : isRowAdded(rowIndex) || isColumnEditable(columnName);
          if (!editable) return;

          const original =
            displayRows[rowIndex]?.[visibleColumnIndices[columnIndex]] ?? null;
          const current = getCellDisplayValue(rowIndex, columnName, original);
          updateCell(rowIndex, columnName, original, nextBooleanValue(current));
          return;
        }

        activateCell(rowIndex, columnName);
      },
      [
        visibleColumns,
        visibleColumnIndices,
        displayRows,
        isRowRemoved,
        isRowAdded,
        isColumnEditable,
        getCellDisplayValue,
        updateCell,
        activateCell,
      ],
    );

    // Click on the index gutter: adds to the selection; Shift selects a range.
    const handleSelectRowGutter = useCallback(
      (rowIndex: number, event: React.MouseEvent) => {
        toggleRowSelection(rowIndex, selectionModeFromGutterEvent(event));
      },
      [toggleRowSelection],
    );

    const { onKeyDown } = useResultsTableKeyboard({
      visibleColumns,
      rowCount: displayRows.length,
      focusedCell,
      setFocusedCell,
      activeCell,
      activateCell: handleActivateCell,
      rowVirtualizer,
      columnVirtualizer,
      containerRef,
      hasMore,
      onLoadMore,
    });

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
      return <LoadingState className={className} />;
    }

    if (displayRows.length === 0) {
      return (
        <div
          className={cn(
            'flex flex-col h-full rounded-md border overflow-hidden',
            className,
          )}
        >
          <EmptyState className="flex-1 min-h-0" message={emptyMessage} />
          <DataTableStatusBar
            executionTimeMs={executionTimeMs}
            rowCount={rowCount}
            rowsLength={0}
            totalCount={totalCount}
            isEditable={isEditable}
            editableInfo={editableInfo}
            changesCount={changesCount}
            addedCount={addedCount}
            removedCount={removedCount}
            pendingCount={pendingCount}
            isLoadingMore={isLoadingMore ?? false}
            hasMore={hasMore ?? false}
            viewMode={viewMode}
            onViewModeChange={setViewMode}
            onAddRow={addRow}
            onDiscardChanges={discardChanges}
            onSave={save}
          />
        </div>
      );
    }

    return (
      <div
        className={cn(
          'flex flex-col h-full overflow-hidden border-t border-border',
          className,
        )}
      >
        <div className="flex-1 min-h-0">
          {viewMode === 'vertical' ? (
            <ResultsTableVertical
              columns={visibleColumns}
              columnIndices={visibleColumnIndices}
              rows={displayRows}
              isPrimaryKeyColumn={isPrimaryKeyColumn}
              isColumnEditable={isColumnEditable}
              isCellModified={isCellModified}
              isRowModified={isRowModified}
              isRowAdded={isRowAdded}
              isRowRemoved={isRowRemoved}
              isRowSelected={isRowSelected}
              isColumnSelected={isColumnSelected}
              getCellDisplayValue={getCellDisplayValue}
              updateCell={updateCell}
              activeCell={activeCell}
              onActivateCell={activateCell}
              onCloseCell={deactivateCell}
              onSelectRow={handleSelectRowGutter}
              onSelectColumn={toggleColumnSelection}
              hasMore={hasMore}
              isLoadingMore={isLoadingMore}
              onLoadMore={onLoadMore}
            />
          ) : (
            <div
              ref={containerRef}
              tabIndex={0}
              onKeyDown={onKeyDown}
              className="relative h-full w-full overflow-auto scrollbar-thin outline-none"
            >
              {/* Header — sticky gutter + only visible columns */}
              <div
                className="bg-background sticky top-0 z-20 border-b border-border"
                style={{
                  width: `${contentWidth}px`,
                  height: `${HEADER_HEIGHT}px`,
                }}
              >
                <div
                  className="sticky left-0 z-30 bg-sidebar border-r border-border flex items-center justify-center text-[10px] text-muted-foreground"
                  style={{
                    width: `${GUTTER_WIDTH}px`,
                    height: `${HEADER_HEIGHT}px`,
                  }}
                >
                  #
                </div>
                {virtualColumns.map(virtualColumn => {
                  const column = visibleColumns[virtualColumn.index];
                  if (!column) return null;

                  return (
                    <div
                      key={virtualColumn.key}
                      className="absolute top-0"
                      style={{
                        left: `${GUTTER_WIDTH + virtualColumn.start}px`,
                        width: `${virtualColumn.size}px`,
                        height: `${HEADER_HEIGHT}px`,
                      }}
                    >
                      <ColumnCell
                        column={column}
                        isPrimaryKeyColumn={isPrimaryKeyColumn(column.name)}
                        isSorted={activeSort?.column === column.name}
                        sortDirection={
                          activeSort?.column === column.name
                            ? activeSort.direction
                            : undefined
                        }
                        onSort={onSort}
                        className="w-full h-full"
                      />
                      {/* Alça de resize: arraste muda a largura desta coluna
                          no header e no corpo (mesmo virtualizador) */}
                      <div
                        onMouseDown={event =>
                          handleResizeStart(
                            event,
                            column.name,
                            virtualColumn.size,
                          )
                        }
                        className="absolute top-0 right-0 z-10 h-full w-1.5 cursor-col-resize select-none hover:bg-primary/60 active:bg-primary"
                      />
                    </div>
                  );
                })}
              </div>

              {/* Body — only visible rows × visible columns */}
              <div
                className="relative"
                style={{
                  width: `${contentWidth}px`,
                  height: `${rowVirtualizer.getTotalSize()}px`,
                }}
              >
                {virtualRows.map(virtualRow => {
                  const row = displayRows[virtualRow.index];
                  if (!row) return null;

                  return (
                    <ResultsTableRowCell
                      key={virtualRow.key}
                      row={row}
                      rowIndex={virtualRow.index}
                      isModified={isRowModified(virtualRow.index)}
                      isAdded={isRowAdded(virtualRow.index)}
                      isRemoved={isRowRemoved(virtualRow.index)}
                      isSelected={isRowSelected(virtualRow.index)}
                      isEven={virtualRow.index % 2 === 0}
                      rowHeight={ROW_HEIGHT}
                      rowStart={virtualRow.start}
                      gutterWidth={GUTTER_WIDTH}
                      totalWidth={contentWidth}
                      columns={visibleColumns}
                      columnIndices={visibleColumnIndices}
                      virtualColumns={virtualColumns}
                      getCellDisplayValue={getCellDisplayValue}
                      isCellModified={isCellModified}
                      isColumnEditable={isColumnEditable}
                      updateCell={updateCell}
                      activeColumnName={
                        activeCell?.rowIndex === virtualRow.index
                          ? activeCell.columnName
                          : null
                      }
                      focusedColumnName={
                        focusedCell?.rowIndex === virtualRow.index
                          ? focusedCell.columnName
                          : null
                      }
                      onActivateCell={handleActivateCell}
                      onCloseCell={deactivateCell}
                      onFocusCell={handleFocusCell}
                      onSelectRowBody={handleSelectRowBody}
                      onSelectRowGutter={handleSelectRowGutter}
                    />
                  );
                })}
              </div>
            </div>
          )}
        </div>

        <DataTableStatusBar
          executionTimeMs={executionTimeMs}
          rowCount={rowCount}
          rowsLength={rows.length}
          totalCount={totalCount}
          isEditable={isEditable}
          editableInfo={editableInfo}
          changesCount={changesCount}
          addedCount={addedCount}
          removedCount={removedCount}
          pendingCount={pendingCount}
          isLoadingMore={isLoadingMore ?? false}
          hasMore={hasMore ?? false}
          viewMode={viewMode}
          onViewModeChange={setViewMode}
          onAddRow={addRow}
          onDiscardChanges={discardChanges}
          onSave={save}
        />
      </div>
    );
  },
);
