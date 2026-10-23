import { describe, expect, it } from 'vitest';
import { treeKey } from './use-tree-navigation';

describe('treeKey', () => {
  it('hjkl e o viram as setas e o Enter', () => {
    expect(['j', 'k', 'l', 'h', 'o'].map(treeKey)).toEqual([
      'ArrowDown',
      'ArrowUp',
      'ArrowRight',
      'ArrowLeft',
      'Enter',
    ]);
  });

  it('as outras teclas (e as maiúsculas) passam como vieram', () => {
    expect(
      ['ArrowDown', 'Enter', '/', 'Escape', 'J', 'x'].map(treeKey),
    ).toEqual(['ArrowDown', 'Enter', '/', 'Escape', 'J', 'x']);
  });
});
