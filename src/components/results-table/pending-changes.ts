import type {
  QueryColumnInfo,
  RowEdit,
  RowInsert,
} from '@/api/types/query.types';
import type { AddedRow, ChangesMap } from './results-table.types';

/**
 * As regras das alterações pendentes da grade (DESIGN.md §2.4), sem React:
 * o `use-results-table` guarda o estado e chama estas funções, que são o que
 * vai para o backend no `onSave`.
 */

type PkValues = (string | null)[];

/**
 * Identidade de uma linha existente pelos valores da chave primária. Em JSON,
 * e não juntando com um separador: um valor com `|` ou o texto `"NULL"` não
 * pode se confundir com outra linha nem com NULL.
 */
export const encodePkKey = (values: PkValues): string => JSON.stringify(values);

export const decodePkKey = (key: string): PkValues =>
  JSON.parse(key) as PkValues;

/** A chave da linha (`null` sem PK conhecida) */
export function pkKeyOf(
  row: (string | null)[],
  pkIndices: number[] | undefined,
): string | null {
  if (!pkIndices?.length) return null;
  return encodePkKey(pkIndices.map(index => row[index] ?? null));
}

/**
 * Registra (ou desfaz) a edição de uma célula de linha existente. Voltar ao
 * valor original apaga a alteração; a comparação distingue `''` de NULL.
 * Devolve um mapa novo — nunca mexe no anterior.
 */
export function applyCellChange(
  changes: ChangesMap,
  pkKey: string,
  columnId: string,
  originalValue: string | null,
  newValue: string | null,
): ChangesMap {
  const next = new Map(changes);
  const rowChanges = new Map(changes.get(pkKey));

  if (newValue === originalValue) {
    if (!rowChanges.delete(columnId)) return changes;
  } else {
    rowChanges.set(columnId, { columnId, originalValue, newValue });
  }

  if (rowChanges.size === 0) next.delete(pkKey);
  else next.set(pkKey, rowChanges);
  return next;
}

/** UPDATEs: uma entrada por linha editada, fora as que serão removidas */
export function buildEdits(
  changes: ChangesMap,
  removedRowKeys: ReadonlySet<string>,
): RowEdit[] {
  const edits: RowEdit[] = [];
  changes.forEach((rowChanges, pkKey) => {
    if (removedRowKeys.has(pkKey)) return;
    edits.push({
      pkValues: decodePkKey(pkKey),
      changes: Array.from(rowChanges.values(), change => [
        change.columnId,
        change.newValue,
      ]),
    });
  });
  return edits;
}

/**
 * INSERTs das linhas novas. Coluna não tocada fica de fora (o banco aplica o
 * default); tocada entra mesmo com NULL. Linha nova sem nada tocado não vira
 * INSERT.
 */
export function buildInserts(
  addedRows: AddedRow[],
  columns: QueryColumnInfo[],
): RowInsert[] {
  return addedRows
    .map(row => ({
      values: row.cells.flatMap((value, index): [string, string | null][] =>
        row.touched.has(index) ? [[columns[index].name, value]] : [],
      ),
    }))
    .filter(insert => insert.values.length > 0);
}

/** DELETEs: os valores da PK de cada linha marcada */
export const buildDeletes = (removedRowKeys: ReadonlySet<string>): PkValues[] =>
  Array.from(removedRowKeys, decodePkKey);

/** Células editadas que ainda contam (linha removida não conta a edição) */
export function countCellChanges(
  changes: ChangesMap,
  removedRowKeys: ReadonlySet<string>,
): number {
  let count = 0;
  changes.forEach((rowChanges, pkKey) => {
    if (!removedRowKeys.has(pkKey)) count += rowChanges.size;
  });
  return count;
}
