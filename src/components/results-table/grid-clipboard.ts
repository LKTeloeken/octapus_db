import type { QueryColumnInfo } from '@/api/types/query.types';
import type { CopiedCell } from '@/stores/grid-clipboard-store';
import type { AddedRow } from '@/stores/pending-changes-store';
import type { CellPosition } from './results-table.types';

/**
 * As regras de copiar/colar da grade, sem React: o texto que vai para a área
 * de transferência e as linhas novas que uma colagem (ou duplicação) cria.
 */

/** Separador entre as células de uma mesma linha no texto copiado */
export const CELL_SEPARATOR = ', ';

/** Aspas só quando a célula tem o separador, aspas ou quebra de linha */
const quoteCell = (value: string) =>
  /[,"\n\r]/.test(value) ? `"${value.replace(/"/g, '""')}"` : value;

/**
 * O texto de um bloco de células. Uma célula sozinha vai crua (NULL vira
 * vazio), como o valor que ela mostra. Mais de uma: linhas da grade em linhas
 * do texto, células separadas por vírgula e NULL escrito — senão a posição
 * de um NULL se perderia na lista.
 */
export function formatCellsText(matrix: CopiedCell[][]): string {
  if (matrix.length === 1 && matrix[0].length === 1) {
    return matrix[0][0] ?? '';
  }
  return matrix
    .map(row =>
      row
        .map(value => (value == null ? 'NULL' : quoteCell(value)))
        .join(CELL_SEPARATOR),
    )
    .join('\n');
}

/** Retângulo de células, em índices de linha e de coluna visível */
export interface CellRange {
  top: number;
  bottom: number;
  left: number;
  right: number;
}

/**
 * O retângulo entre a célula de origem (`anchor`) e o cursor. Sem origem não há
 * bloco: a seleção é só a célula do cursor. Coluna que sumiu (ocultada depois)
 * desfaz o bloco.
 */
export function cellRangeOf(
  anchor: CellPosition | null,
  focused: CellPosition | null,
  visibleColumns: QueryColumnInfo[],
): CellRange | null {
  if (!anchor || !focused) return null;
  const from = visibleColumns.findIndex(c => c.name === anchor.columnName);
  const to = visibleColumns.findIndex(c => c.name === focused.columnName);
  if (from === -1 || to === -1) return null;
  return {
    top: Math.min(anchor.rowIndex, focused.rowIndex),
    bottom: Math.max(anchor.rowIndex, focused.rowIndex),
    left: Math.min(from, to),
    right: Math.max(from, to),
  };
}

const newTempId = () =>
  typeof crypto !== 'undefined' && 'randomUUID' in crypto
    ? crypto.randomUUID()
    : `${Date.now()}-${Math.random()}`;

/**
 * Linhas novas a partir de linhas copiadas, casando as colunas pelo nome.
 * Coluna que a origem não tem, célula sem valor (`undefined`) e coluna em
 * `leaveDefault` ficam "não tocadas": o INSERT as omite e o banco aplica o
 * default. Linha que não casa nenhuma coluna não entra.
 */
export function buildAddedRows(
  source: { columns: string[]; rows: CopiedCell[][] },
  columns: QueryColumnInfo[],
  leaveDefault: ReadonlySet<string>,
): AddedRow[] {
  const sourceIndex = new Map(source.columns.map((name, i) => [name, i]));

  return source.rows.flatMap(row => {
    const touched = new Set<number>();
    const cells = columns.map((column, index) => {
      const from = sourceIndex.get(column.name);
      if (from === undefined || leaveDefault.has(column.name)) return null;
      const value = row[from];
      if (value === undefined) return null;
      touched.add(index);
      return value;
    });
    return touched.size > 0 ? [{ tempId: newTempId(), cells, touched }] : [];
  });
}
