import type { RowChanges, RowEdit, RowInsert } from '@/api/types/query.types';

/** As alterações pendentes da grade (o `SaveRowChanges` da ResultsTable) */
export type PendingRowChanges = RowChanges;

/** As operações de gravação da grade (as mutations do React Query) */
export interface RowChangeOperations {
  /**
   * Tudo numa transação só (`save_row_changes`). Quem chama passa só quando o
   * banco tem `supportsAtomicSave`; sem ela, cada etapa vai numa chamada.
   */
  saveAll?: (changes: PendingRowChanges) => Promise<{ affectedRows: number }>;
  deleteRows: (
    pkValues: (string | null)[][],
  ) => Promise<{ affectedRows: number }>;
  insertRows: (rows: RowInsert[]) => Promise<{ affectedRows: number }>;
  applyEdits: (edits: RowEdit[]) => Promise<{ affectedRows: number }>;
}

/**
 * Grava as alterações pendentes na ordem de BACKEND.md §6.6: DELETE → INSERT
 * → UPDATE (remover primeiro libera chaves que uma linha nova pode reusar).
 * Com `saveAll`, o backend faz as três numa transação: se uma falha, nada
 * vale e a grade, que mantém tudo pendente, pode reenviar igual. Sem ela,
 * cada etapa é uma transação própria e as anteriores à falha já valeram.
 * Devolve o total de linhas afetadas.
 */
export async function persistRowChanges(
  changes: PendingRowChanges,
  operations: RowChangeOperations,
): Promise<number> {
  const { edits, inserts, deletes } = changes;
  if (deletes.length === 0 && inserts.length === 0 && edits.length === 0) {
    return 0;
  }
  if (operations.saveAll) {
    return (await operations.saveAll(changes)).affectedRows;
  }

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
