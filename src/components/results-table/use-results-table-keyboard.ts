import type { Virtualizer } from '@tanstack/react-virtual';
import { useCallback, useEffect, useRef, type RefObject } from 'react';
import type { QueryColumnInfo } from '@/api/types/query.types';
import { useShortcuts } from '@/lib/keybindings/use-shortcuts';
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
  /** Move o cursor mantendo a origem do bloco de células (Shift+setas) */
  extendCellSelection: (rowIndex: number, columnName: string) => void;
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
  extendCellSelection,
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
    (rowIndex: number, columnIndex: number, extend = false) => {
      const column = visibleColumns[columnIndex];
      if (!column) return;

      if (extend) extendCellSelection(rowIndex, column.name);
      else setFocusedCell({ rowIndex, columnName: column.name });
      rowVirtualizer.scrollToIndex(rowIndex, { align: 'auto' });
      columnVirtualizer.scrollToIndex(columnIndex, { align: 'auto' });
    },
    [
      visibleColumns,
      setFocusedCell,
      extendCellSelection,
      rowVirtualizer,
      columnVirtualizer,
    ],
  );

  /**
   * Uma ação de navegação a partir da célula com o cursor. Sem cursor ainda,
   * a primeira tecla pousa na primeira célula. Um refetch pode ter encolhido a
   * tabela ou escondido a coluna — sempre partimos de uma posição válida.
   */
  const fromCursor =
    (
      action: (position: {
        rowIndex: number;
        columnIndex: number;
        lastRow: number;
        lastColumn: number;
      }) => void,
    ) =>
    () => {
      if (!focusedCell) {
        moveTo(0, 0);
        return;
      }
      const current: CellPosition = focusedCell;
      action({
        rowIndex: clamp(current.rowIndex, rowCount - 1),
        columnIndex: Math.max(
          visibleColumns.findIndex(
            column => column.name === current.columnName,
          ),
          0,
        ),
        lastRow: rowCount - 1,
        lastColumn: visibleColumns.length - 1,
      });
    };

  // O Tab não passa por aqui: ele troca a visualização (`useViewToggle`).
  const { onKeyDown } = useShortcuts(
    {
      'grid.down': fromCursor(({ rowIndex, columnIndex, lastRow }) => {
        // Bater no fim com mais páginas disponíveis puxa o próximo lote.
        if (rowIndex === lastRow) {
          if (hasMore) onLoadMore?.();
          return;
        }
        moveTo(rowIndex + 1, columnIndex);
      }),
      'grid.up': fromCursor(({ rowIndex, columnIndex }) =>
        moveTo(Math.max(rowIndex - 1, 0), columnIndex),
      ),
      'grid.right': fromCursor(({ rowIndex, columnIndex, lastColumn }) =>
        moveTo(rowIndex, Math.min(columnIndex + 1, lastColumn)),
      ),
      'grid.left': fromCursor(({ rowIndex, columnIndex }) =>
        moveTo(rowIndex, Math.max(columnIndex - 1, 0)),
      ),
      'grid.extendDown': fromCursor(({ rowIndex, columnIndex, lastRow }) =>
        moveTo(Math.min(rowIndex + 1, lastRow), columnIndex, true),
      ),
      'grid.extendUp': fromCursor(({ rowIndex, columnIndex }) =>
        moveTo(Math.max(rowIndex - 1, 0), columnIndex, true),
      ),
      'grid.extendRight': fromCursor(({ rowIndex, columnIndex, lastColumn }) =>
        moveTo(rowIndex, Math.min(columnIndex + 1, lastColumn), true),
      ),
      'grid.extendLeft': fromCursor(({ rowIndex, columnIndex }) =>
        moveTo(rowIndex, Math.max(columnIndex - 1, 0), true),
      ),
      'grid.rowStart': fromCursor(({ rowIndex }) => moveTo(rowIndex, 0)),
      'grid.rowEnd': fromCursor(({ rowIndex, lastColumn }) =>
        moveTo(rowIndex, lastColumn),
      ),
      'grid.edit': fromCursor(({ rowIndex, columnIndex }) =>
        activateCell(rowIndex, visibleColumns[columnIndex].name),
      ),
    },
    {
      target: 'element',
      // Enquanto um editor está aberto ele manda no teclado (o conteúdo do
      // Popover é portal, mas o boolean é inline e fica dentro do container).
      when: () => !activeCell && visibleColumns.length > 0 && rowCount > 0,
    },
  );

  return { onKeyDown };
};
