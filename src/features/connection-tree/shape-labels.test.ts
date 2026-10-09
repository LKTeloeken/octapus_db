import { describe, expect, it } from 'vitest';
import type { ShapeGroup } from '@/api/types/catalog.types';
import { driftBadge, shapeGroupRow } from './shape-labels';

const group = (overrides: Partial<ShapeGroup>): ShapeGroup => ({
  key: 'abc',
  role: 'dominant',
  tables: 150,
  missing: [],
  extra: [],
  schemas: 4950,
  ...overrides,
});

describe('driftBadge', () => {
  it('sem diferença não há aviso', () => {
    expect(driftBadge(null)).toBeUndefined();
    expect(driftBadge({ missing: 0, extra: 0 })).toBeUndefined();
  });

  it('usa o sinal de menos e explica no title', () => {
    expect(driftBadge({ missing: 3, extra: 0 })).toEqual({
      label: '−3',
      title: 'Fora do formato principal: faltam 3 tabelas',
    });
    expect(driftBadge({ missing: 2, extra: 1 })).toEqual({
      label: '−2 +1',
      title: 'Fora do formato principal: faltam 2 tabelas e sobram 1 tabela',
    });
  });
});

describe('shapeGroupRow', () => {
  it('molde e resto com a contagem de schemas', () => {
    expect(shapeGroupRow(group({}))).toEqual({
      name: 'Formato principal',
      subLabel: '4.950',
    });
    expect(
      shapeGroupRow(
        group({ key: 'other', role: 'other', tables: null, schemas: 2 }),
      ),
    ).toEqual({ name: 'Outros schemas', subLabel: '2' });
  });

  it('variação lista o que falta e o que sobra (até 10 nomes)', () => {
    const missing = Array.from({ length: 12 }, (_, i) => `t${i + 1}`);
    const row = shapeGroupRow(
      group({
        role: 'variant',
        tables: 139,
        missing,
        extra: ['legacy'],
        schemas: 50,
      }),
    );
    expect(row.name).toBe('Variação');
    expect(row.subLabel).toBe('50');
    expect(row.badge?.label).toBe('−12 +1');
    expect(row.badge?.title).toBe(
      [
        'Variação com 139 tabelas',
        'Faltam: t1, t2, t3, t4, t5, t6, t7, t8, t9, t10 e mais 2',
        'Sobram: legacy',
      ].join('\n'),
    );
  });
});
