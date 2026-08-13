import type { Virtualizer } from '@tanstack/react-virtual';
import { useCallback, useEffect, useRef, type KeyboardEvent, type RefObject } from 'react';
import type { QueryColumnInfo } from '@/api/types/query.types';
import { useFocusStore } from '@/stores/focus-store';
import type {
  ActivateCellFn,
  ActiveCell,
  CellPosition,
  FocusedCell,
} from './results-table.types';

interface Options {
  /** Colunas efetivamente desenhadas (ocultas já filtradas) */
  visibleColumns: QueryColumnInfo[];
  rowCount: number;
  focusedCell: FocusedCell;
  setFocusedCell: (cell: FocusedCell) => void;
  activeCell: ActiveCell;
  activateCell: ActivateCellFn;
  rowVirtualizer: Virtualizer<HTMLDivElement, Element>;
  columnVirtualizer: Virtualizer<HTMLDivElement, Element>;
  containerRef: RefObject<HTMLDivElement | null>;
  hasMore?: boolean;
  onLoadMore?: () => void;
}

const clamp = (value: number, max: number) =>
  Math.min(Math.max(value, 0), Math.max(max, 0));

/**
 * Navegação por teclado da grade: um cursor de célula que anda com as setas e
 * abre o editor no Enter. Fica separado de `useResultsTable` porque a conta do
 * movimento depende de `visibleColumns` e dos virtualizadores, que só existem
 * no componente.
 */
export const useResultsTableKeyboard = ({
  visibleColumns,
  rowCount,
  focusedCell,
  setFocusedCell,
  activeCell,
  activateCell,
  rowVirtualizer,
  columnVirtualizer,
  containerRef,
  hasMore,
  onLoadMore,
}: Options) => {
  const focusRequest = useFocusStore(state => state.request);
  const clearFocusRequest = useFocusStore(state => state.clearFocusRequest);

  // Quando o editor (Popover, em portal) fecha, o foco do DOM ficaria perdido
  // no body e as setas parariam de funcionar — devolvemos para o container.
  const hadActiveCell = useRef(false);
  useEffect(() => {
    if (activeCell) {
      hadActiveCell.current = true;
      return;
    }
    if (hadActiveCell.current) {
      hadActiveCell.current = false;
      containerRef.current?.focus();
    }
  }, [activeCell, containerRef]);

  // Pedido de foco vindo da árvore (abrir uma tabela, Cmd/Ctrl+→). O container
  // só existe depois que os dados chegam, por isso `rowCount` entra nas deps:
  // o pedido fica pendente e é consumido no render em que a grade aparece.
  useEffect(() => {
    if (focusRequest?.panel !== 'grid') return;
    if (!containerRef.current) return;
    containerRef.current.focus();
    clearFocusRequest();
  }, [focusRequest, rowCount, containerRef, clearFocusRequest]);

  const moveTo = useCallback(
    (rowIndex: number, columnIndex: number) => {
      const column = visibleColumns[columnIndex];
      if (!column) return;

      setFocusedCell({ rowIndex, columnName: column.name });
      rowVirtualizer.scrollToIndex(rowIndex, { align: 'auto' });
      columnVirtualizer.scrollToIndex(columnIndex, { align: 'auto' });
    },
    [visibleColumns, setFocusedCell, rowVirtualizer, columnVirtualizer],
  );

  const onKeyDown = useCallback(
    (event: KeyboardEvent) => {
      // Acordes com modificador pertencem aos atalhos globais (Cmd/Ctrl+←/→
      // trocam de painel, Cmd/Ctrl+S salva) — Shift fica de fora por causa do
      // Shift+Tab.
      if (event.metaKey || event.ctrlKey || event.altKey) return;

      // Enquanto um editor está aberto ele manda no teclado (o conteúdo do
      // Popover é portal, mas o boolean é inline e fica dentro do container).
      if (activeCell) return;
      if (visibleColumns.length === 0 || rowCount === 0) return;

      const navKeys = [
        'ArrowDown',
        'ArrowUp',
        'ArrowLeft',
        'ArrowRight',
        'Home',
        'End',
        'Tab',
        'Enter',
      ];
      if (!navKeys.includes(event.key)) return;

      // Sem cursor ainda: a primeira tecla pousa na primeira célula. O Tab fica
      // de fora para continuar servindo de saída da grade.
      if (!focusedCell) {
        if (event.key === 'Tab') return;
        event.preventDefault();
        moveTo(0, 0);
        return;
      }

      // Um refetch pode ter encolhido a tabela ou escondido a coluna — sempre
      // partimos de uma posição válida.
      const current: CellPosition = focusedCell;
      const rowIndex = clamp(current.rowIndex, rowCount - 1);
      const columnIndex = Math.max(
        visibleColumns.findIndex(column => column.name === current.columnName),
        0,
      );
      const lastRow = rowCount - 1;
      const lastColumn = visibleColumns.length - 1;

      switch (event.key) {
        case 'ArrowDown':
          event.preventDefault();
          // Bater no fim com mais páginas disponíveis puxa o próximo lote.
          if (rowIndex === lastRow) {
            if (hasMore) onLoadMore?.();
            return;
          }
          moveTo(rowIndex + 1, columnIndex);
          return;

        case 'ArrowUp':
          event.preventDefault();
          moveTo(Math.max(rowIndex - 1, 0), columnIndex);
          return;

        case 'ArrowRight':
          event.preventDefault();
          moveTo(rowIndex, Math.min(columnIndex + 1, lastColumn));
          return;

        case 'ArrowLeft':
          event.preventDefault();
          moveTo(rowIndex, Math.max(columnIndex - 1, 0));
          return;

        case 'Home':
          event.preventDefault();
          moveTo(rowIndex, 0);
          return;

        case 'End':
          event.preventDefault();
          moveTo(rowIndex, lastColumn);
          return;

        // Tab anda célula a célula, quebrando para a linha seguinte/anterior.
        // Nas pontas ele NÃO é interceptado: sem isso a grade viraria uma
        // armadilha de foco e não daria para sair dela pelo teclado.
        case 'Tab': {
          if (event.shiftKey) {
            if (columnIndex > 0) {
              event.preventDefault();
              moveTo(rowIndex, columnIndex - 1);
            } else if (rowIndex > 0) {
              event.preventDefault();
              moveTo(rowIndex - 1, lastColumn);
            }
            return;
          }
          if (columnIndex < lastColumn) {
            event.preventDefault();
            moveTo(rowIndex, columnIndex + 1);
          } else if (rowIndex < lastRow) {
            event.preventDefault();
            moveTo(rowIndex + 1, 0);
          }
          return;
        }

        case 'Enter':
          event.preventDefault();
          activateCell(rowIndex, visibleColumns[columnIndex].name);
          return;
      }
    },
    [
      activeCell,
      visibleColumns,
      rowCount,
      focusedCell,
      moveTo,
      activateCell,
      hasMore,
      onLoadMore,
    ],
  );

  return { onKeyDown };
};
