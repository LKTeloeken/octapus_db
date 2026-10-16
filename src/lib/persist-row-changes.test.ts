import { describe, expect, it } from 'vitest';
import {
  persistRowChanges,
  type RowChangeOperations,
} from './persist-row-changes';

/** Operações falsas que anotam a ordem e afetam `affected` linhas cada */
function fakeOperations(affected = 1, failOn?: keyof RowChangeOperations) {
  const calls: string[] = [];
  const op =
    (name: keyof RowChangeOperations) =>
    async (): Promise<{ affectedRows: number }> => {
      calls.push(name);
      if (name === failOn) throw new Error(`${name} falhou`);
      return { affectedRows: affected };
    };
  const operations: RowChangeOperations = {
    deleteRows: op('deleteRows'),
    insertRows: op('insertRows'),
    applyEdits: op('applyEdits'),
  };
  return { calls, operations };
}

const all = {
  deletes: [['1']],
  inserts: [{ values: [['nome', 'Eva']] as [string, string | null][] }],
  edits: [
    {
      pkValues: ['2'],
      changes: [['nome', 'Bia']] as [string, string | null][],
    },
  ],
};

describe('persistRowChanges', () => {
  it('grava na ordem DELETE → INSERT → UPDATE e soma as linhas', async () => {
    const { calls, operations } = fakeOperations(2);
    await expect(persistRowChanges(all, operations)).resolves.toBe(6);
    expect(calls).toEqual(['deleteRows', 'insertRows', 'applyEdits']);
  });

  it('pula o que não tem nada pendente', async () => {
    const { calls, operations } = fakeOperations();
    await persistRowChanges({ ...all, deletes: [], inserts: [] }, operations);
    expect(calls).toEqual(['applyEdits']);
  });

  it('uma etapa que falha para o resto e o erro sobe', async () => {
    const { calls, operations } = fakeOperations(1, 'insertRows');
    await expect(persistRowChanges(all, operations)).rejects.toThrow(
      'insertRows falhou',
    );
    // O DELETE já foi; o UPDATE nem começou
    expect(calls).toEqual(['deleteRows', 'insertRows']);
  });
});
