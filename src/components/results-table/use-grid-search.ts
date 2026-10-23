import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { QueryColumnInfo } from '@/api/types/query.types';
import { useShortcuts } from '@/lib/keybindings/use-shortcuts';
import { ownsKeyboardEvent } from './keyboard-scope';
import {
  EMPTY_MATCHES,
  compileMatcher,
  createScanState,
  finishScan,
  lowerBoundMatch,
  matchAt,
  scanSlice,
  yieldToMain,
  type CellTextReader,
  type GridMatcher,
  type GridMatches,
  type GridSearchOptions,
  type SearchColumn,
} from '@/lib/grid-search';
import {
  getGridSearch,
  useGridSearchStore,
  type GridSearchCurrent,
} from '@/stores/grid-search-store';
import {
  isBooleanTrue,
  resolveCellEditor,
} from './results-table-cell/use-resolve-cell-editor';
import type {
  DataTableRow,
  GetCellDisplayValueFn,
} from './results-table.types';

/** Espera depois da última tecla antes de varrer de novo */
const DEBOUNCE_MS = 150;
/** Orçamento de cada fatia da varredura — folga para o frame de 16 ms */
const SLICE_MS = 5;

interface Options {
  /** Id da aba: o estado da busca é por aba */
  scopeId: string;
  /** Região de teclado da grade (`keyboard-scope.ts`); `null` = a da aba */
  keyboardScope?: string | null;
  /** Linhas desenhadas (carregadas + adicionadas pendentes) */
  rows: DataTableRow[];
  /** Colunas desenhadas (ocultas já fora) */
  columns: QueryColumnInfo[];
  /** Índice original em `row` de cada coluna de `columns` */
  columnIndices: number[];
  getCellDisplayValue: GetCellDisplayValueFn;
  /** Cursor da grade: sem resultado atual, a busca parte dele */
  origin: GridSearchCurrent | null;
  /** Leva o cursor (e a rolagem) até o resultado */
  onNavigate: (rowIndex: number, visibleColumnIndex: number) => void;
  /** O painel fechou: devolver o foco para a grade */
  onClosed: () => void;
}

interface ScanResult {
  matcher: GridMatcher;
  matches: GridMatches;
}

/**
 * Busca na grade (Cmd/Ctrl+F). Estado de UI por aba no `grid-search-store`;
 * aqui ficam o atalho, o debounce, a varredura fatiada e a navegação.
 *
 * Performance: a varredura completa (todas as linhas carregadas) só serve para
 * contar e navegar e roda em fatias de ~5 ms, cancelada a cada mudança. O
 * destaque não depende dela — cada célula desenhada aplica o `matcher` em si
 * mesma, então só as ~300 células visíveis pagam o custo dos trechos.
 */
export const useGridSearch = ({
  scopeId,
  keyboardScope = null,
  rows,
  columns,
  columnIndices,
  getCellDisplayValue,
  origin,
  onNavigate,
  onClosed,
}: Options) => {
  const search = useGridSearchStore(state =>
    getGridSearch(state.byScope, scopeId),
  );
  const toggleInStore = useGridSearchStore(state => state.toggle);
  const setOpenInStore = useGridSearchStore(state => state.setOpen);
  const setQueryInStore = useGridSearchStore(state => state.setQuery);
  const setOptionsInStore = useGridSearchStore(state => state.setOptions);
  const setCurrentInStore = useGridSearchStore(state => state.setCurrent);

  const { open: isOpen, query, options, current } = search;
  const inputRef = useRef<HTMLInputElement>(null);

  // ── Debounce, amarrado à aba ──────────────────────────────────────────
  // A grade é a mesma instância ao trocar entre abas do mesmo tipo: o valor
  // "atrasado" de outra aba não vale aqui, então a aba nova usa o texto já.
  const [debounced, setDebounced] = useState({ scopeId, query });
  const effectiveQuery =
    debounced.scopeId === scopeId ? debounced.query : query;

  useEffect(() => {
    if (debounced.scopeId === scopeId && debounced.query === query) return;
    const delay =
      debounced.scopeId !== scopeId || query === '' ? 0 : DEBOUNCE_MS;
    const timer = setTimeout(() => setDebounced({ scopeId, query }), delay);
    return () => clearTimeout(timer);
  }, [scopeId, query, debounced]);

  // ── Casamento ─────────────────────────────────────────────────────────

  const compiled = useMemo(
    () =>
      isOpen
        ? compileMatcher(effectiveQuery, options)
        : { matcher: null, error: null },
    [isOpen, effectiveQuery, options],
  );
  const matcher = compiled.matcher;

  // Colunas que entram na busca, na ordem da grade.
  const searchColumns = useMemo<SearchColumn[]>(() => {
    const listed = new Set(options.columns);
    const result: SearchColumn[] = [];
    columns.forEach((column, visibleIndex) => {
      if (options.columnScope === 'only' && !listed.has(column.name)) return;
      if (options.columnScope === 'except' && listed.has(column.name)) return;
      result.push({ visibleIndex, name: column.name });
    });
    return result;
  }, [columns, options.columnScope, options.columns]);

  const searchColumnNames = useMemo(
    () => new Set(searchColumns.map(column => column.name)),
    [searchColumns],
  );

  // O texto que a célula mostra: valor com edição pendente, booleano como
  // `true`/`false` (o Postgres devolve "t"/"f" cru). NULL nunca casa.
  const read = useMemo<CellTextReader>(() => {
    const isBoolean = columns.map(
      column => resolveCellEditor(column.typeName) === 'boolean',
    );
    return (rowIndex, column) => {
      const raw = rows[rowIndex]?.[columnIndices[column.visibleIndex]] ?? null;
      const text = getCellDisplayValue(rowIndex, column.name, raw);
      if (text === null) return null;
      if (isBoolean[column.visibleIndex]) {
        return isBooleanTrue(text) ? 'true' : 'false';
      }
      return text;
    };
  }, [rows, columns, columnIndices, getCellDisplayValue]);

  // ── Varredura fatiada ─────────────────────────────────────────────────

  const [result, setResult] = useState<ScanResult | null>(null);
  const [isScanning, setIsScanning] = useState(false);

  useEffect(() => {
    if (!matcher) {
      setIsScanning(false);
      return;
    }

    let cancelled = false;
    const state = createScanState();
    setIsScanning(true);

    const run = async () => {
      for (;;) {
        scanSlice(
          state,
          rows.length,
          searchColumns,
          read,
          matcher,
          performance.now() + SLICE_MS,
        );
        if (state.done) break;
        await yieldToMain();
        if (cancelled) return;
      }
      setResult({ matcher, matches: finishScan(state) });
      setIsScanning(false);
    };
    void run();

    return () => {
      cancelled = true;
    };
  }, [matcher, rows.length, searchColumns, read]);

  // Linhas novas/edições re-varrem com o mesmo matcher: o resultado anterior
  // continua valendo até o novo chegar (sem piscar o contador). Matcher novo
  // invalida na hora.
  const matches =
    matcher && result?.matcher === matcher ? result.matches : EMPTY_MATCHES;
  const hasFreshResult = !!matcher && result?.matcher === matcher;

  // ── Navegação ─────────────────────────────────────────────────────────

  const columnIndexByName = useMemo(() => {
    const map = new Map<string, number>();
    columns.forEach((column, index) => map.set(column.name, index));
    return map;
  }, [columns]);

  const positionOf = useCallback(
    (cell: GridSearchCurrent | null) => {
      if (!cell) return null;
      const column = columnIndexByName.get(cell.columnName);
      return column === undefined ? null : { row: cell.rowIndex, column };
    },
    [columnIndexByName],
  );

  // Índice do resultado atual, ou -1 quando a posição guardada não é (mais)
  // um resultado — um refetch ou uma edição pode tê-la tirado da lista.
  const currentIndex = useMemo(() => {
    const position = positionOf(current);
    if (!position || matches.count === 0) return -1;
    const index = lowerBoundMatch(matches, position.row, position.column);
    if (index >= matches.count) return -1;
    const match = matchAt(matches, index);
    return match.rowIndex === position.row &&
      match.visibleColumnIndex === position.column
      ? index
      : -1;
  }, [current, matches, positionOf]);

  const goTo = useCallback(
    (index: number) => {
      if (matches.count === 0) return;
      const wrapped = ((index % matches.count) + matches.count) % matches.count;
      const match = matchAt(matches, wrapped);
      const column = columns[match.visibleColumnIndex];
      if (!column) return;
      setCurrentInStore(scopeId, {
        rowIndex: match.rowIndex,
        columnName: column.name,
      });
      onNavigate(match.rowIndex, match.visibleColumnIndex);
    },
    [matches, columns, scopeId, setCurrentInStore, onNavigate],
  );

  // Sem resultado atual válido, navegar parte do cursor da grade.
  const startPosition = useCallback(
    () => positionOf(current) ?? positionOf(origin),
    [current, origin, positionOf],
  );

  const next = useCallback(() => {
    if (matches.count === 0) return;
    if (currentIndex >= 0) return goTo(currentIndex + 1);
    const position = startPosition();
    goTo(
      position ? lowerBoundMatch(matches, position.row, position.column) : 0,
    );
  }, [matches, currentIndex, startPosition, goTo]);

  const prev = useCallback(() => {
    if (matches.count === 0) return;
    const position = startPosition();
    if (!position) return goTo(matches.count - 1);
    goTo(lowerBoundMatch(matches, position.row, position.column) - 1);
  }, [matches, startPosition, goTo]);

  // Busca nova (texto/opções): pula para o primeiro resultado a partir do
  // atual, como o Cmd+F do editor. Só re-varreduras por linhas/edições não
  // mexem no cursor.
  const jumpedFor = useRef<GridMatcher | null>(null);
  useEffect(() => {
    if (!hasFreshResult || !matcher || jumpedFor.current === matcher) return;
    jumpedFor.current = matcher;
    if (matches.count === 0) return;
    const position = startPosition();
    const index = position
      ? lowerBoundMatch(matches, position.row, position.column)
      : 0;
    goTo(index >= matches.count ? 0 : index);
  }, [hasFreshResult, matcher, matches, startPosition, goTo]);

  // ── Abrir / fechar ────────────────────────────────────────────────────

  const toggle = useCallback(
    () => toggleInStore(scopeId),
    [toggleInStore, scopeId],
  );
  const close = useCallback(
    () => setOpenInStore(scopeId, false),
    [setOpenInStore, scopeId],
  );

  // Abrir foca o campo com o texto selecionado; fechar devolve o foco à
  // grade. Só reage à transição dentro da mesma aba — voltar para uma aba com
  // o painel aberto não rouba o foco.
  const wasOpen = useRef({ scopeId, open: isOpen });
  useEffect(() => {
    const previous = wasOpen.current;
    wasOpen.current = { scopeId, open: isOpen };
    if (previous.scopeId !== scopeId || previous.open === isOpen) return;
    if (isOpen) {
      inputRef.current?.focus();
      inputRef.current?.select();
    } else {
      onClosed();
    }
  }, [scopeId, isOpen, onClosed]);

  // Cmd/Ctrl+F alterna; Cmd/Ctrl+G / Shift+Cmd/Ctrl+G navegam. Dentro de um
  // CodeMirror (editor SQL, painel de valor) o atalho é dele. Só a aba ativa
  // monta a grade, então nunca há dois listeners disputando.
  useShortcuts(
    {
      'grid.search': toggle,
      'grid.searchNext': () => {
        if (!isOpen) return false;
        next();
      },
      'grid.searchPrev': () => {
        if (!isOpen) return false;
        prev();
      },
    },
    {
      when: event => {
        const target = event.target as HTMLElement | null;
        if (!ownsKeyboardEvent(target, keyboardScope)) return false;
        if (target?.closest('.cm-editor')) return false;
        const inPanel = !!target?.closest('[data-grid-search]');
        return (
          inPanel ||
          !target?.closest(
            '[data-slot="dialog-content"], [data-slot="popover-content"]',
          )
        );
      },
    },
  );

  // ── Saída para a grade e para o painel ────────────────────────────────

  const active = isOpen ? matcher : null;

  // Matcher só nas colunas do escopo; identidade muda só com a busca, então
  // as células (memo) não re-renderizam à toa.
  const getColumnMatcher = useCallback(
    (columnName: string) =>
      active && searchColumnNames.has(columnName) ? active : null,
    [active, searchColumnNames],
  );

  const currentMatch = active && currentIndex >= 0 ? current : null;

  const setQuery = useCallback(
    (value: string) => setQueryInStore(scopeId, value),
    [setQueryInStore, scopeId],
  );
  const setOptions = useCallback(
    (patch: Partial<GridSearchOptions>) => setOptionsInStore(scopeId, patch),
    [setOptionsInStore, scopeId],
  );

  return {
    isOpen,
    query,
    setQuery,
    options,
    setOptions,
    close,
    inputRef,
    error: compiled.error,
    /** Sem texto (ou regex inválida) não há o que contar */
    hasQuery: !!matcher,
    isScanning: isScanning || (!!matcher && !hasFreshResult),
    /** Colunas no escopo; 0 com "Somente" sem nenhuma marcada */
    searchColumnCount: searchColumns.length,
    matchCount: matches.count,
    capped: matches.capped,
    currentIndex,
    next,
    prev,
    getColumnMatcher,
    currentMatch,
  };
};

export type UseGridSearchReturn = ReturnType<typeof useGridSearch>;
