import { describe, expect, it } from 'vitest';
import {
  DEFAULT_GRID_SEARCH_OPTIONS,
  MAX_GRID_MATCHES,
  MAX_RANGES_PER_CELL,
  compileMatcher,
  createScanState,
  finishScan,
  lowerBoundMatch,
  matchAt,
  scanSlice,
  type GridSearchOptions,
  type SearchColumn,
} from './grid-search';

const compile = (query: string, options: Partial<GridSearchOptions> = {}) =>
  compileMatcher(query, { ...DEFAULT_GRID_SEARCH_OPTIONS, ...options });

const matcherFor = (
  query: string,
  options: Partial<GridSearchOptions> = {},
) => {
  const { matcher, error } = compile(query, options);
  if (!matcher) throw new Error(error ?? 'sem matcher');
  return matcher;
};

describe('compileMatcher', () => {
  it('query vazia não gera matcher nem erro', () => {
    expect(compile('')).toEqual({ matcher: null, error: null });
  });

  it('contém: casa parcial, sem diferenciar maiúsculas por padrão', () => {
    const m = matcherFor('pro');
    expect(m.test('Produto')).toBe(true);
    expect(m.test('Cadeira')).toBe(false);
    expect(m.ranges('Pro e PROMO')).toEqual([
      [0, 3],
      [6, 9],
    ]);
  });

  it('diferenciar maiúsculas', () => {
    const m = matcherFor('Pro', { caseSensitive: true });
    expect(m.test('produto')).toBe(false);
    expect(m.ranges('Pro pro')).toEqual([[0, 3]]);
  });

  it('ignora acentos nos dois sentidos, com offsets do texto original', () => {
    expect(matcherFor('acao').test('Ação')).toBe(true);
    expect(matcherFor('ação').test('ACAO')).toBe(true);
    expect(matcherFor('acao', { ignoreAccents: false }).test('ação')).toBe(
      false,
    );

    // NFD: "é" decomposto ocupa 2 unidades no original.
    const decomposed = 'café cafe';
    expect(matcherFor('cafe').ranges(decomposed)).toEqual([
      [0, 5],
      [6, 10],
    ]);
  });

  it('palavra inteira respeita pontuação e letras acentuadas', () => {
    const m = matcherFor('ana', { mode: 'word' });
    expect(m.test('ana.silva@x.com')).toBe(true);
    expect(m.test('banana')).toBe(false);
    expect(m.test('anaís')).toBe(false);
    expect(m.ranges('banana ana')).toEqual([[7, 10]]);
  });

  it('célula inteira e começa com', () => {
    const exact = matcherFor('ativo', { mode: 'exact' });
    expect(exact.test('Ativo')).toBe(true);
    expect(exact.test('inativo')).toBe(false);
    expect(exact.ranges('ATIVO')).toEqual([[0, 5]]);

    const prefix = matcherFor('ACS', { mode: 'prefix' });
    expect(prefix.test('acs-0001')).toBe(true);
    expect(prefix.test('x-acs')).toBe(false);
  });

  it('regex: casa, ignora casamentos vazios e reporta padrão inválido', () => {
    const m = matcherFor('\\d{3}', { mode: 'regex' });
    expect(m.ranges('a123b4567')).toEqual([
      [1, 4],
      [5, 8],
    ]);
    expect(matcherFor('x*', { mode: 'regex' }).test('abc')).toBe(false);

    const invalid = compile('(', { mode: 'regex' });
    expect(invalid.matcher).toBeNull();
    expect(invalid.error).toBeTruthy();
  });

  it('regex não baixa o padrão (\\D continua sendo não-dígito)', () => {
    const m = matcherFor('\\D+', { mode: 'regex' });
    expect(m.ranges('12ab')).toEqual([[2, 4]]);
  });

  it('limita os trechos por célula', () => {
    const m = matcherFor('a');
    expect(m.ranges('a'.repeat(100))).toHaveLength(MAX_RANGES_PER_CELL);
  });
});

describe('scanSlice', () => {
  const rows = [
    ['Produto', 'pro'],
    ['Cadeira', null],
    ['Monitor', 'PRO-1'],
  ];
  const columns: SearchColumn[] = [
    { visibleIndex: 0, name: 'nome' },
    { visibleIndex: 1, name: 'sku' },
  ];
  const read = (row: number, column: SearchColumn) =>
    rows[row][column.visibleIndex];

  it('varre na ordem da grade e pula NULL', () => {
    const state = createScanState();
    scanSlice(state, rows.length, columns, read, matcherFor('pro'), Infinity);
    expect(state.done).toBe(true);

    const matches = finishScan(state);
    expect(matches.count).toBe(3);
    expect(matchAt(matches, 0)).toEqual({ rowIndex: 0, visibleColumnIndex: 0 });
    expect(matchAt(matches, 1)).toEqual({ rowIndex: 0, visibleColumnIndex: 1 });
    expect(matchAt(matches, 2)).toEqual({ rowIndex: 2, visibleColumnIndex: 1 });
  });

  it('para no prazo e continua de onde parou', () => {
    const many = Array.from({ length: 200 }, () => ['pro']);
    const state = createScanState();
    let clock = 0;
    const m = matcherFor('pro');
    const cols = [{ visibleIndex: 0, name: 'a' }];
    const readMany = (row: number) => many[row][0];

    scanSlice(state, many.length, cols, readMany, m, 1, () => ++clock);
    expect(state.done).toBe(false);
    expect(state.row).toBe(64);

    scanSlice(state, many.length, cols, readMany, m, Infinity);
    expect(state.done).toBe(true);
    expect(state.count).toBe(200);
  });

  it('para no teto e marca como cortado', () => {
    const state = createScanState();
    const count = MAX_GRID_MATCHES + 5;
    scanSlice(
      state,
      count,
      [{ visibleIndex: 0, name: 'a' }],
      () => 'x',
      matcherFor('x'),
      Infinity,
    );
    expect(state.count).toBe(MAX_GRID_MATCHES);
    expect(state.capped).toBe(true);
  });
});

describe('lowerBoundMatch', () => {
  it('acha o primeiro resultado na posição ou depois dela', () => {
    const state = createScanState();
    const cells = [
      [0, 2],
      [3, 0],
      [3, 4],
      [7, 1],
    ];
    const lookup = new Set(cells.map(([r, c]) => `${r}:${c}`));
    scanSlice(
      state,
      8,
      [0, 1, 2, 3, 4].map(i => ({ visibleIndex: i, name: String(i) })),
      (row, column) =>
        lookup.has(`${row}:${column.visibleIndex}`) ? 'x' : null,
      matcherFor('x'),
      Infinity,
    );
    const matches = finishScan(state);

    expect(lowerBoundMatch(matches, 0, 0)).toBe(0);
    expect(lowerBoundMatch(matches, 3, 0)).toBe(1);
    expect(lowerBoundMatch(matches, 3, 1)).toBe(2);
    expect(lowerBoundMatch(matches, 8, 0)).toBe(4);
  });
});
