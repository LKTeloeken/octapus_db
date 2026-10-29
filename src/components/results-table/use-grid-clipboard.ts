import { readText, writeText } from '@tauri-apps/plugin-clipboard-manager';
import type { RefObject } from 'react';
import toast from 'react-hot-toast';
import type { QueryColumnInfo } from '@/api/types/query.types';
import { useShortcuts } from '@/lib/keybindings/use-shortcuts';
import {
  useGridClipboardStore,
  type CopiedCell,
} from '@/stores/grid-clipboard-store';
import { formatCellsText, type CellRange } from './grid-clipboard';
import { ownsGridShortcut } from './keyboard-scope';
import type { FocusedCell, UseResultsTableReturn } from './results-table.types';

const TOAST_ID = 'grid-clipboard';

const linesLabel = (count: number) =>
  `${count} ${count === 1 ? 'linha' : 'linhas'}`;

/** O sistema pode trocar `\n` por `\r\n` e aparar o fim ao guardar o texto */
const sameText = (a: string, b: string) =>
  a.replace(/\r\n/g, '\n').trimEnd() === b.replace(/\r\n/g, '\n').trimEnd();

interface Options extends Pick<
  UseResultsTableReturn,
  | 'isEditable'
  | 'getLastSelection'
  | 'selectedRowIndices'
  | 'rowCells'
  | 'pasteRows'
> {
  /** A grade inteira: copiar/colar só com o foco dentro dela */
  rootRef: RefObject<HTMLElement | null>;
  scopeId: string;
  keyboardScope: string | null;
  columns: QueryColumnInfo[];
  visibleColumns: QueryColumnInfo[];
  visibleColumnIndices: number[];
  focusedCell: FocusedCell;
  cellRange: CellRange | null;
}

/**
 * Copiar e colar na grade. Cmd/Ctrl+C copia, pela última seleção feita:
 * linhas marcadas no gutter (inteiras — só elas colam noutra aba), o bloco de
 * células ou a célula do cursor. Cmd/Ctrl+V cola as linhas copiadas como
 * linhas novas. Num campo de texto as duas teclas continuam nativas.
 */
export const useGridClipboard = ({
  rootRef,
  scopeId,
  keyboardScope,
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
}: Options) => {
  const setCopied = useGridClipboardStore(state => state.setCopied);

  const write = async (text: string, message: string) => {
    try {
      await writeText(text);
      toast.success(message, { id: TOAST_ID });
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error), {
        id: TOAST_ID,
      });
    }
  };

  // O texto mostra só as colunas visíveis, na ordem da grade
  const visibleCells = (cells: CopiedCell[], left = 0, right = -1) =>
    visibleColumnIndices
      .slice(left, right < 0 ? undefined : right + 1)
      .map(index => cells[index]);

  const copy = () => {
    const rows = getLastSelection() === 'rows' ? selectedRowIndices() : [];
    if (rows.length > 0) {
      const full = rows.map(rowCells);
      const text = formatCellsText(full.map(cells => visibleCells(cells)));
      setCopied({
        text,
        scopeId,
        columns: columns.map(c => c.name),
        rows: full,
      });
      void write(text, `${linesLabel(rows.length)} copiadas`);
      return;
    }

    const column = focusedCell
      ? visibleColumns.findIndex(c => c.name === focusedCell.columnName)
      : -1;
    const range =
      cellRange ??
      (focusedCell && column !== -1
        ? {
            top: focusedCell.rowIndex,
            bottom: focusedCell.rowIndex,
            left: column,
            right: column,
          }
        : null);
    if (!range) return false;

    const matrix: CopiedCell[][] = [];
    for (let row = range.top; row <= range.bottom; row++) {
      matrix.push(visibleCells(rowCells(row), range.left, range.right));
    }
    // Células soltas não colam como linhas: a cópia anterior deixa de valer
    setCopied(null);
    const count = matrix.reduce((sum, row) => sum + row.length, 0);
    void write(
      formatCellsText(matrix),
      count === 1 ? 'Valor copiado' : `${count} valores copiados`,
    );
  };

  const paste = async () => {
    const copied = useGridClipboardStore.getState().copied;
    const text = await readText().catch(() => null);
    if (!copied || text === null || !sameText(text, copied.text)) {
      toast.error(
        'Para colar linhas, copie-as inteiras de uma grade (marque pelo número da linha)',
        { id: TOAST_ID },
      );
      return;
    }
    const count = pasteRows(copied);
    if (count === 0) {
      toast.error('Nenhuma coluna das linhas copiadas existe nesta tabela', {
        id: TOAST_ID,
      });
      return;
    }
    toast.success(`${linesLabel(count)} coladas como novas`, { id: TOAST_ID });
  };

  useShortcuts(
    {
      'grid.copy': copy,
      'grid.paste': () => {
        if (!isEditable) return false;
        void paste();
      },
    },
    {
      // Diferente dos outros atalhos da grade, não vale fora dela: o Cmd/Ctrl+C
      // na árvore ou no assistente não pode copiar a célula da aba.
      when: event =>
        ownsGridShortcut(event, keyboardScope) &&
        event.target instanceof Node &&
        rootRef.current?.contains(event.target) === true,
    },
  );
};
