import { describe, expect, it } from 'vitest';
import type { QueryColumnInfo } from '@/api/types/query.types';
import { buildAddedRows, cellRangeOf, formatCellsText } from './grid-clipboard';

const col = (name: string): QueryColumnInfo => ({
  name,
  typeName: 'text',
  typeOid: null,
});
const COLUMNS = ['id', 'name', 'email', 'created_at'].map(col);

describe('formatCellsText', () => {
  it('uma célula vai crua; NULL vira vazio', () => {
    expect(formatCellsText([['Maria, Silva']])).toBe('Maria, Silva');
    expect(formatCellsText([[null]])).toBe('');
  });

  it('várias células: vírgula entre elas, linha por linha, NULL escrito', () => {
    expect(
      formatCellsText([
        ['1', 'Ana', null],
        ['2', 'Bia "B"', 'a, b'],
      ]),
    ).toBe('1, Ana, NULL\n2, "Bia ""B""", "a, b"');
  });
});

describe('cellRangeOf', () => {
  it('normaliza os cantos e some sem origem', () => {
    const anchor = { rowIndex: 5, columnName: 'email' };
    const focused = { rowIndex: 2, columnName: 'id' };
    expect(cellRangeOf(anchor, focused, COLUMNS)).toEqual({
      top: 2,
      bottom: 5,
      left: 0,
      right: 2,
    });
    expect(cellRangeOf(null, focused, COLUMNS)).toBeNull();
    expect(
      cellRangeOf(anchor, { rowIndex: 1, columnName: 'oculta' }, COLUMNS),
    ).toBeNull();
  });
});

describe('buildAddedRows', () => {
  const source = {
    columns: ['id', 'email', 'name', 'extra'],
    rows: [['7', 'a@x.com', undefined, 'z']],
  };

  it('casa pelo nome; default, sem valor e coluna ausente ficam não tocados', () => {
    const [row] = buildAddedRows(source, COLUMNS, new Set(['id']));
    expect(row.cells).toEqual([null, null, 'a@x.com', null]);
    expect(Array.from(row.touched)).toEqual([2]);
  });

  it('sem colunas de default, leva a PK junto', () => {
    const [row] = buildAddedRows(source, COLUMNS, new Set());
    expect(row.cells[0]).toBe('7');
    expect(row.touched.has(0)).toBe(true);
  });

  it('linha que não casa nenhuma coluna não entra', () => {
    expect(
      buildAddedRows({ columns: ['outra'], rows: [['x']] }, COLUMNS, new Set()),
    ).toEqual([]);
  });
});
