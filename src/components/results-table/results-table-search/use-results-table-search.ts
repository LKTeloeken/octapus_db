import { useCallback, useMemo, useState, type KeyboardEvent } from 'react';
import { formatCount } from '@/lib/format-count';
import { DEFAULT_GRID_SEARCH_OPTIONS } from '@/lib/grid-search';
import type { ResultsTableSearchProps } from './results-table-search.types';

/** Lógica do painel de busca: texto do status, opções e teclado do campo */
export const useResultsTableSearch = ({
  search,
  columns,
}: ResultsTableSearchProps) => {
  const {
    options,
    setOptions,
    close,
    next,
    prev,
    error,
    hasQuery,
    isScanning,
    matchCount,
    capped,
    currentIndex,
    searchColumnCount,
  } = search;

  const [showOptions, setShowOptions] = useState(false);
  const [columnFilter, setColumnFilter] = useState('');

  const status = useMemo(() => {
    if (error || !hasQuery) return null;
    if (searchColumnCount === 0) return 'Marque as colunas';
    if (matchCount === 0) return isScanning ? 'buscando…' : 'Nenhum resultado';
    const total = `${formatCount(matchCount)}${capped ? '+' : ''}`;
    return currentIndex >= 0
      ? `${formatCount(currentIndex + 1)} de ${total}`
      : `${total} resultado${matchCount === 1 ? '' : 's'}`;
  }, [
    error,
    hasQuery,
    searchColumnCount,
    matchCount,
    isScanning,
    capped,
    currentIndex,
  ]);

  // Quantas opções saem do padrão — vira o número no botão de opções.
  const activeOptionsCount =
    Number(options.mode !== DEFAULT_GRID_SEARCH_OPTIONS.mode) +
    Number(
      options.caseSensitive !== DEFAULT_GRID_SEARCH_OPTIONS.caseSensitive,
    ) +
    Number(
      options.ignoreAccents !== DEFAULT_GRID_SEARCH_OPTIONS.ignoreAccents,
    ) +
    Number(options.columnScope !== 'all');

  const listedColumns = useMemo(
    () => new Set(options.columns),
    [options.columns],
  );

  const filteredColumns = useMemo(() => {
    const term = columnFilter.trim().toLowerCase();
    if (!term) return columns;
    return columns.filter(column => column.name.toLowerCase().includes(term));
  }, [columns, columnFilter]);

  const toggleColumn = useCallback(
    (name: string) => {
      const nextColumns = new Set(options.columns);
      if (nextColumns.has(name)) nextColumns.delete(name);
      else nextColumns.add(name);
      setOptions({ columns: Array.from(nextColumns) });
    },
    [options.columns, setOptions],
  );

  // Enter/Shift+Enter navegam; Esc fecha. Os outros atalhos (Cmd+F, Cmd+G)
  // vêm do listener global do `useGridSearch`.
  const onInputKeyDown = useCallback(
    (event: KeyboardEvent<HTMLInputElement>) => {
      if (event.key === 'Enter') {
        event.preventDefault();
        if (event.shiftKey) prev();
        else next();
      }
    },
    [next, prev],
  );

  const onPanelKeyDown = useCallback(
    (event: KeyboardEvent<HTMLDivElement>) => {
      if (event.key !== 'Escape') return;
      event.preventDefault();
      event.stopPropagation();
      close();
    },
    [close],
  );

  return {
    status,
    showOptions,
    toggleOptions: () => setShowOptions(value => !value),
    activeOptionsCount,
    columnFilter,
    setColumnFilter,
    filteredColumns,
    isColumnListed: (name: string) => listedColumns.has(name),
    toggleColumn,
    onInputKeyDown,
    onPanelKeyDown,
  };
};
