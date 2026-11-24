import { describe, expect, it } from 'vitest';
import { computeTreeGuides } from './tree-guides';

describe('computeTreeGuides', () => {
  it('liga irmãos e curva no último filho', () => {
    // servidor
    //  ├ banco a
    //  │  ╰ tabela
    //  ╰ banco b
    expect(computeTreeGuides([0, 1, 2, 1])).toEqual([
      [],
      ['tee'],
      ['line', 'end'],
      ['end'],
    ]);
  });

  it('não continua a linha de um ancestro que já acabou', () => {
    expect(computeTreeGuides([0, 1, 2, 2, 0])).toEqual([
      [],
      ['end'],
      ['none', 'tee'],
      ['none', 'end'],
      [],
    ]);
  });
});
