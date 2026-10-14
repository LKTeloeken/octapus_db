import { beforeEach, describe, expect, it } from 'vitest';
import { DEFAULT_GRID_SEARCH_OPTIONS } from '@/lib/grid-search';
import { getGridSearch, useGridSearchStore } from './grid-search-store';

const stateOf = (scopeId: string) =>
  getGridSearch(useGridSearchStore.getState().byScope, scopeId);

describe('grid-search-store', () => {
  beforeEach(() => {
    useGridSearchStore.setState({ byScope: new Map() });
  });

  it('aba sem estado devolve o padrão fechado', () => {
    expect(stateOf('a')).toEqual({
      open: false,
      query: '',
      options: DEFAULT_GRID_SEARCH_OPTIONS,
      current: null,
    });
  });

  it('cada aba tem o próprio painel, texto e opções', () => {
    const { toggle, setQuery, setOptions } = useGridSearchStore.getState();
    toggle('a');
    setQuery('a', 'pro');
    setOptions('a', { mode: 'exact' });
    setQuery('b', 'outra');

    expect(stateOf('a')).toMatchObject({ open: true, query: 'pro' });
    expect(stateOf('a').options.mode).toBe('exact');
    expect(stateOf('b')).toMatchObject({ open: false, query: 'outra' });
    expect(stateOf('b').options.mode).toBe('contains');
  });

  it('toggle alterna e clear remove só a aba pedida', () => {
    const { toggle, clear } = useGridSearchStore.getState();
    toggle('a');
    toggle('b');
    toggle('b');
    expect(stateOf('a').open).toBe(true);
    expect(stateOf('b').open).toBe(false);

    clear('a');
    expect(useGridSearchStore.getState().byScope.has('a')).toBe(false);
    expect(useGridSearchStore.getState().byScope.has('b')).toBe(true);
  });
});
