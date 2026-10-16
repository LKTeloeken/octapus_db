import type { RowEdit, RowInsert } from '@/api/types/query.types';

/** As alterações pendentes da grade (o `SaveRowChanges` da ResultsTable) */
export interface PendingRowChanges {
  edits: RowEdit[];
  inserts: RowInsert[];
  deletes: (string | null)[][];
}

/** As três operações de gravação da grade (as mutations do React Query) */
export interface RowChangeOperations {
  deleteRows: (
    pkValues: (string | null)[][],
  ) => Promise<{ affectedRows: number }>;
  insertRows: (rows: RowInsert[]) => Promise<{ affectedRows: number }>;
  applyEdits: (edits: RowEdit[]) => Promise<{ affectedRows: number }>;
}

/**
 * Grava as alterações pendentes na ordem de BACKEND.md §6.6: DELETE → INSERT
 * → UPDATE (remover primeiro libera chaves que uma linha nova pode reusar).
 * Cada etapa é uma transação própria no backend: se uma falha, o erro sobe e
 * as anteriores já valeram — a grade mantém tudo pendente para tentar de novo.
 * Devolve o total de linhas afetadas.
 */
export async function persistRowChanges(
  { edits, inserts, deletes }: PendingRowChanges,
  operations: RowChangeOperations,
): Promise<number> {
  let affected = 0;
  if (deletes.length > 0) {
    affected += (await operations.deleteRows(deletes)).affectedRows;
  }
  if (inserts.length > 0) {
    affected += (await operations.insertRows(inserts)).affectedRows;
  }
  if (edits.length > 0) {
    affected += (await operations.applyEdits(edits)).affectedRows;
  }
  return affected;
}
