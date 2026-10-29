import { cn } from '@/lib/utils';
import type {
  ResultsTableProps,
  RowSelectionMode,
} from './results-table.types';
import { memo, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { QueryColumnInfo } from '@/api/types/query.types';
import { ExportDialog } from '@/components/export-dialog/export-dialog';
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from '@/components/ui/resizable';
import ColumnCell from './results-table-column-cell/results-table-column-cell';
import {
  nextBooleanValue,
  resolveCellEditor,
} from './results-table-cell/use-resolve-cell-editor';
import useResultsTable from './use-results-table';
import { useResultsTableKeyboard } from './use-results-table-keyboard';
import { useViewToggle } from './use-view-toggle';
import { useVirtualizer } from '@tanstack/react-virtual';
import { DataTableStatusBar } from './results-table-status-bar/results-table-status-bar';
import { EmptyState } from './empty-state';
import { LoadingState } from './loading-state';
import { ResultsTablePendingBar } from './results-table-pending-bar/results-table-pending-bar';
import { ResultsTableRowCell } from './results-table-row-cell/results-table-row-cell';
import { ResultsTableVertical } from './results-table-vertical/results-table-vertical';
import { ResultsTableValuePanel } from './results-table-value-panel/results-table-value-panel';
import { ResultsTableSearch } from './results-table-search/results-table-search';
import { KEYBOARD_SCOPE_ATTR } from './keyboard-scope';
import { cellRangeOf } from './grid-clipboard';
import { useGridClipboard } from './use-grid-clipboard';
import { useGridSearch } from './use-grid-search';
import type { ValuePanelTarget } from './results-table-value-panel/results-table-value-panel.types';

// Densidade compacta do Ink (DESIGN.md §4): linha de 28, header de 40.
const ROW_HEIGHT = 28;
const HEADER_HEIGHT = 40;
const COLUMN_WIDTH = 192;
const MIN_COLUMN_WIDTH = 60;
const GUTTER_WIDTH = 44;
const OVERSCAN_Y = 10;
const OVERSCAN_X = 3;
/** Respiro no fim da grade enquanto a barra de pendências flutua sobre ela */
const PENDING_BAR_SPACE = 64;

/** Modo de seleção de um clique no corpo da linha: Cmd/Ctrl alterna a linha.
 *  Sem modificador o clique só move o cursor; com Shift estende o bloco de
 *  células (tratado na célula) — a faixa de linhas é Shift no gutter. */
const selectionModeFromEvent = (
  event: React.MouseEvent,
): RowSelectionMode | null => {
  if (event.shiftKey) return null;
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
    onFetchAllRows,
    exportFileName,
    defaultColumns,
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
    showValuePanel,
    onCloseValuePanel,
    searchScopeId,
    keyboardScope = false,
    className,
  }: ResultsTableProps) => {
    // Região de teclado própria: só com várias grades na tela (assistente)
    const ownScope = keyboardScope ? searchScopeId : null;
    const scopeProps = ownScope ? { [KEYBOARD_SCOPE_ATTR]: ownScope } : {};
    const rootRef = useRef<HTMLDivElement>(null);
    const containerRef = useRef<HTMLDivElement>(null);
    const [isExportOpen, setIsExportOpen] = useState(false);

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
      isCellDefault,
      isRowAdded,
      isRowRemoved,
      isRowSelected,
      isColumnSelected,
      selectedRows,
      getCellDisplayValue,
      activeCell,
      activateCell,
      deactivateCell,
      focusedCell,
      focusCell,
      setFocusedCell,
      cellAnchor,
      extendCellSelection,
      getLastSelection,
      selectedRowIndices,
      rowCells,
      duplicateRows,
      pasteRows,
      toggleRowSelection,
      toggleAllRowsSelection,
      allRowsSelected,
      toggleColumnSelection,
      addRow,
      discardChanges,
      save,
      changesCount,
      addedCount,
      removedCount,
      pendingCount,
    } = useResultsTable(
      rows,
      columns,
      editableInfo,
      onSave,
      searchScopeId,
      ownScope,
      defaultColumns,
    );

    const { viewMode, setViewMode, viewsRef, onViewKeyDown } = useViewToggle(
      activeCell !== null,
    );

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

    // O bloco de células selecionado (Shift+clique/setas, arraste), em índices
    // de coluna visível; null quando a seleção é só a célula do cursor.
    const cellRange = useMemo(
      () => cellRangeOf(cellAnchor, focusedCell, visibleColumns),
      [cellAnchor, focusedCell, visibleColumns],
    );

    useGridClipboard({
      rootRef,
      scopeId: searchScopeId,
      keyboardScope: ownScope,
      columns,
      visibleColumns,
      visibleColumnIndices,
      focusedCell,
      cellRange,
      isEditable,
      getLastSelection,
      selectedRowIndices,
      rowCells,
      pasteRows,
    });

    // A exportação leva o que está na grade: colunas visíveis, na ordem delas,
    // carregando o índice original para ler a célula da linha crua.
    const exportColumns = useMemo(
      () =>
        visibleColumns.map((column, index) => ({
          name: column.name,
          typeName: column.typeName,
          index: visibleColumnIndices[index],
        })),
      [visibleColumns, visibleColumnIndices],
    );

    // A célula do painel de valor é a do cursor de teclado. Busca no array
    // completo de colunas: ocultar a coluna depois de focá-la não esvazia o
    // painel.
    const valuePanelTarget = useMemo<ValuePanelTarget | null>(() => {
      if (!showValuePanel || !focusedCell) return null;

      const { rowIndex, columnName } = focusedCell;
      const columnIndex = columns.findIndex(c => c.name === columnName);
      const column = columns[columnIndex];
      const row = displayRows[rowIndex];
      // Um refetch pode ter encolhido a tabela sob o cursor.
      if (!column || !row) return null;

      const originalValue = row[columnIndex] ?? null;
      return {
        rowIndex,
        column,
        originalValue,
        value: getCellDisplayValue(rowIndex, columnName, originalValue),
        // Mesma regra da grade: linha nova edita tudo, removida congela.
        isEditable: isRowRemoved(rowIndex)
          ? false
          : isRowAdded(rowIndex) || isColumnEditable(columnName),
        isModified: isCellModified(rowIndex, columnName),
      };
    }, [
      showValuePanel,
      focusedCell,
      columns,
      displayRows,
      getCellDisplayValue,
      isRowRemoved,
      isRowAdded,
      isColumnEditable,
      isCellModified,
    ]);

    // Esc no editor do painel devolve o teclado para a grade.
    const focusGrid = useCallback(() => containerRef.current?.focus(), []);

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

    // Ir a um resultado da busca: move o cursor de célula (o painel de valor
    // acompanha) e rola até ele. Na visão vertical quem rola é ela mesma, a
    // partir do `currentMatch`.
    const handleSearchNavigate = useCallback(
      (rowIndex: number, visibleColumnIndex: number) => {
        const column = visibleColumns[visibleColumnIndex];
        if (!column) return;
        setFocusedCell({ rowIndex, columnName: column.name });
        if (viewMode !== 'table') return;
        // Fora da vista → centraliza: com 'auto' a célula pararia na borda,
        // e a do canto superior direito ficaria embaixo do painel da busca.
        // Já visível → não rola (navegar não deve sacudir a grade).
        const el = containerRef.current;
        if (!el) return;
        const scrollIntoCenter = (
          virtualizer: typeof rowVirtualizer,
          index: number,
          offset: number,
          size: number,
          leading: number,
        ) => {
          const item = virtualizer.measurementsCache[index];
          if (!item) return;
          const visible =
            item.start >= offset && item.end <= offset + size - leading;
          if (!visible) virtualizer.scrollToIndex(index, { align: 'center' });
        };
        scrollIntoCenter(
          rowVirtualizer,
          rowIndex,
          el.scrollTop,
          el.clientHeight,
          HEADER_HEIGHT,
        );
        scrollIntoCenter(
          columnVirtualizer,
          visibleColumnIndex,
          el.scrollLeft,
          el.clientWidth,
          GUTTER_WIDTH,
        );
      },
      [
        visibleColumns,
        setFocusedCell,
        viewMode,
        rowVirtualizer,
        columnVirtualizer,
      ],
    );

    const search = useGridSearch({
      scopeId: searchScopeId,
      keyboardScope: ownScope,
      rows: displayRows,
      columns: visibleColumns,
      columnIndices: visibleColumnIndices,
      getCellDisplayValue,
      origin: focusedCell,
      onNavigate: handleSearchNavigate,
      onClosed: focusGrid,
    });
    const { getColumnMatcher, currentMatch } = search;

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
    // container, o único tab stop da grade. O botão segue apertado → arrastar
    // sobre outras células estende o bloco a partir desta.
    const draggingCells = useRef(false);
    useEffect(() => {
      const stop = () => {
        draggingCells.current = false;
      };
      window.addEventListener('mouseup', stop);
      return () => window.removeEventListener('mouseup', stop);
    }, []);

    const handleFocusCell = useCallback(
      (rowIndex: number, columnName: string) => {
        focusCell(rowIndex, columnName);
        draggingCells.current = true;
        containerRef.current?.focus();
      },
      [focusCell],
    );

    const handleExtendCell = useCallback(
      (rowIndex: number, columnName: string) => {
        extendCellSelection(rowIndex, columnName);
        draggingCells.current = true;
        containerRef.current?.focus();
      },
      [extendCellSelection],
    );

    const handleHoverCell = useCallback(
      (rowIndex: number, columnName: string, buttons: number) => {
        // Soltar o botão fora da janela não dispara o mouseup aqui
        if (!draggingCells.current || (buttons & 1) === 0) {
          draggingCells.current = false;
          return;
        }
        extendCellSelection(rowIndex, columnName);
      },
      [extendCellSelection],
    );

    // Linhas novas (Nova linha, Duplicar, Colar) entram no fim da grade: o
    // cursor vai para a primeira delas e a grade rola até lá — sem disparar o
    // "carregar mais" do fim da lista, que empurraria a linha para longe.
    // Remontar com pendências guardadas não conta como "novas".
    const seenAddedCount = useRef(addedCount);
    const revealingAddedUntil = useRef(0);
    useEffect(() => {
      const previous = seenAddedCount.current;
      seenAddedCount.current = addedCount;
      if (addedCount <= previous) return;
      const firstNew = displayRows.length - (addedCount - previous);
      const columnName = focusedCell?.columnName ?? visibleColumns[0]?.name;
      if (columnName) setFocusedCell({ rowIndex: firstNew, columnName });
      if (viewMode !== 'table') return;
      revealingAddedUntil.current = performance.now() + 500;
      // Em px, e não `scrollToIndex`: o virtualizador não conta o cabeçalho
      // fixo e pararia uma linha antes do fim. O navegador limita ao máximo.
      requestAnimationFrame(() => {
        const el = containerRef.current;
        if (!el) return;
        el.scrollTop =
          HEADER_HEIGHT + firstNew * ROW_HEIGHT - el.clientHeight / 2;
      });
    }, [
      addedCount,
      viewMode,
      displayRows.length,
      focusedCell,
      visibleColumns,
      setFocusedCell,
    ]);

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
      extendCellSelection,
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
      if (performance.now() < revealingAddedUntil.current) return;

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
          ref={rootRef}
          {...scopeProps}
          className={cn('flex h-full flex-col overflow-hidden', className)}
        >
          <EmptyState className="flex-1 min-h-0" message={emptyMessage} />
          <DataTableStatusBar
            executionTimeMs={executionTimeMs}
            rowCount={rowCount}
            rowsLength={0}
            totalCount={totalCount}
            isEditable={isEditable}
            editableInfo={editableInfo}
            isLoadingMore={isLoadingMore ?? false}
            hasMore={hasMore ?? false}
            viewMode={viewMode}
            onViewModeChange={setViewMode}
            onAddRow={addRow}
            onExport={() => setIsExportOpen(true)}
          />
        </div>
      );
    }

    return (
      <div
        ref={rootRef}
        {...scopeProps}
        className={cn('flex h-full flex-col overflow-hidden', className)}
      >
        <div ref={viewsRef} className="relative flex-1 min-h-0">
          {/* O grupo existe mesmo com o painel fechado: abrir/fechar só
              acrescenta o irmão, sem remontar a grade (e perder a rolagem). */}
          <ResizablePanelGroup
            direction="horizontal"
            autoSaveId="results-table-value-panel"
          >
            <ResizablePanel
              id="results-grid"
              order={1}
              minSize={25}
              className="relative"
            >
              {search.isOpen && (
                <ResultsTableSearch search={search} columns={visibleColumns} />
              )}
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
                  focusedCell={focusedCell}
                  getColumnMatcher={getColumnMatcher}
                  currentMatch={currentMatch}
                  onActivateCell={activateCell}
                  onCloseCell={deactivateCell}
                  onFocusCell={focusCell}
                  onSelectRow={handleSelectRowGutter}
                  onSelectColumn={toggleColumnSelection}
                  hasMore={hasMore}
                  isLoadingMore={isLoadingMore}
                  onLoadMore={onLoadMore}
                  onKeyDown={onViewKeyDown}
                />
              ) : (
                <div
                  ref={containerRef}
                  data-results-view="table"
                  // Grade virtualizada: as contagens e os índices (aria-rowindex
                  // / aria-colindex, base 1, cabeçalho = linha 1) dizem ao
                  // leitor de tela onde está cada linha montada.
                  role="grid"
                  aria-rowcount={displayRows.length + 1}
                  aria-colcount={visibleColumns.length}
                  tabIndex={0}
                  onKeyDown={event => {
                    onViewKeyDown(event);
                    onKeyDown(event);
                  }}
                  className="relative h-full w-full overflow-auto scrollbar-thin outline-none"
                >
                  {/* Header — sticky gutter + only visible columns */}
                  <div
                    role="row"
                    aria-rowindex={1}
                    className="sticky top-0 z-20 border-b border-line bg-surface-2"
                    style={{
                      width: `${contentWidth}px`,
                      height: `${HEADER_HEIGHT}px`,
                    }}
                  >
                    {/* Canto do gutter: seleciona/limpa todas as linhas */}
                    <button
                      type="button"
                      tabIndex={-1}
                      aria-label={
                        allRowsSelected
                          ? 'Limpar seleção'
                          : 'Selecionar todas as linhas'
                      }
                      title={
                        allRowsSelected
                          ? 'Limpar seleção'
                          : 'Selecionar todas as linhas (Ctrl+A)'
                      }
                      onClick={toggleAllRowsSelection}
                      className={cn(
                        'sticky left-0 z-30 flex cursor-pointer select-none items-center justify-end overflow-hidden border-r border-line-subtle bg-surface-2 px-2 font-mono text-[11px] outline-none transition-colors hover:text-fg',
                        allRowsSelected ? 'text-iris-text' : 'text-fg-subtle',
                      )}
                      style={{
                        width: `${GUTTER_WIDTH}px`,
                        height: `${HEADER_HEIGHT}px`,
                      }}
                    >
                      {allRowsSelected && (
                        <span
                          aria-hidden
                          className="absolute inset-0 pointer-events-none bg-iris-soft"
                        />
                      )}
                      <span className="relative">#</span>
                    </button>
                    {virtualColumns.map(virtualColumn => {
                      const column = visibleColumns[virtualColumn.index];
                      if (!column) return null;

                      return (
                        <div
                          key={virtualColumn.key}
                          role="columnheader"
                          aria-colindex={virtualColumn.index + 1}
                          aria-sort={
                            activeSort?.column === column.name
                              ? activeSort.direction === 'asc'
                                ? 'ascending'
                                : 'descending'
                              : undefined
                          }
                          data-column={column.name}
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
                            className="absolute top-0 right-0 z-10 h-full w-1.5 cursor-col-resize select-none transition-colors hover:bg-line-strong active:bg-ring"
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
                      // A barra de pendências não pode cobrir a última linha
                      marginBottom:
                        pendingCount > 0 ? `${PENDING_BAR_SPACE}px` : undefined,
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
                          rowHeight={ROW_HEIGHT}
                          rowStart={virtualRow.start}
                          gutterWidth={GUTTER_WIDTH}
                          totalWidth={contentWidth}
                          columns={visibleColumns}
                          columnIndices={visibleColumnIndices}
                          virtualColumns={virtualColumns}
                          getCellDisplayValue={getCellDisplayValue}
                          isCellModified={isCellModified}
                          isCellDefault={isCellDefault}
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
                          getColumnMatcher={getColumnMatcher}
                          currentMatchColumnName={
                            currentMatch?.rowIndex === virtualRow.index
                              ? currentMatch.columnName
                              : null
                          }
                          cellRange={
                            cellRange &&
                            virtualRow.index >= cellRange.top &&
                            virtualRow.index <= cellRange.bottom
                              ? cellRange
                              : null
                          }
                          onActivateCell={handleActivateCell}
                          onCloseCell={deactivateCell}
                          onFocusCell={handleFocusCell}
                          onExtendCell={handleExtendCell}
                          onHoverCell={handleHoverCell}
                          onSelectRowBody={handleSelectRowBody}
                          onSelectRowGutter={handleSelectRowGutter}
                        />
                      );
                    })}
                  </div>
                </div>
              )}
            </ResizablePanel>

            {showValuePanel && (
              <>
                <ResizableHandle className="cursor-col-resize! bg-line-subtle" />
                <ResizablePanel
                  id="results-value-panel"
                  order={2}
                  defaultSize={30}
                  minSize={15}
                  maxSize={70}
                >
                  <ResultsTableValuePanel
                    target={valuePanelTarget}
                    updateCell={updateCell}
                    onClose={onCloseValuePanel}
                    onEscape={focusGrid}
                  />
                </ResizablePanel>
              </>
            )}
          </ResizablePanelGroup>

          {/* Pendências flutuam sobre a grade, em vidro, só enquanto existem. */}
          {pendingCount > 0 && (
            <div className="pointer-events-none absolute inset-x-0 bottom-4 z-40 flex justify-center px-4">
              <ResultsTablePendingBar
                changesCount={changesCount}
                addedCount={addedCount}
                removedCount={removedCount}
                onDiscard={discardChanges}
                onSave={save}
              />
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
          isLoadingMore={isLoadingMore ?? false}
          hasMore={hasMore ?? false}
          viewMode={viewMode}
          onViewModeChange={setViewMode}
          onAddRow={addRow}
          onDuplicateRows={duplicateRows}
          onExport={() => setIsExportOpen(true)}
        />

        <ExportDialog
          open={isExportOpen}
          onOpenChange={setIsExportOpen}
          columns={exportColumns}
          rows={rows}
          selectedRows={selectedRows}
          hasMore={hasMore ?? false}
          totalCount={totalCount}
          fileName={exportFileName ?? 'resultado'}
          editableInfo={editableInfo}
          defaultColumns={defaultColumns}
          onFetchAllRows={onFetchAllRows}
        />
      </div>
    );
  },
);
