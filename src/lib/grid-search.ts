/**
 * Busca de texto na grade de resultados (Cmd/Ctrl+F). Código puro, sem React:
 * a compilação do casamento e a varredura fatiada, testáveis isoladamente.
 *
 * Dois caminhos com custos diferentes:
 * - `test` — usado na varredura de TODAS as linhas carregadas (contagem e
 *   navegação). Não monta ranges nem mapeia offsets.
 * - `ranges` — usado só nas células que o virtualizador desenha, para o
 *   destaque. Limitado em caracteres e em quantidade de trechos.
 */

export type GridSearchMode = 'contains' | 'word' | 'exact' | 'prefix' | 'regex';

/** Quais colunas entram: todas, só as listadas, ou todas menos as listadas */
export type GridSearchColumnScope = 'all' | 'only' | 'except';

export interface GridSearchOptions {
  mode: GridSearchMode;
  caseSensitive: boolean;
  /** `ação` casa com `acao` (e vice-versa) */
  ignoreAccents: boolean;
  columnScope: GridSearchColumnScope;
  /** Nomes de coluna do escopo `only`/`except`; ignorado em `all` */
  columns: string[];
}

export const DEFAULT_GRID_SEARCH_OPTIONS: GridSearchOptions = {
  mode: 'contains',
  caseSensitive: false,
  ignoreAccents: true,
  columnScope: 'all',
  columns: [],
};

/** Trecho casado, `[início, fim)` em offsets do texto original */
export type MatchRange = readonly [number, number];

export interface GridMatcher {
  test: (text: string) => boolean;
  ranges: (text: string) => MatchRange[];
}

export type CompiledSearch =
  | { matcher: GridMatcher; error: null }
  | { matcher: null; error: string | null };

/** O resto do valor está cortado pelo `truncate` da célula: não vale destacar */
export const MAX_HIGHLIGHT_CHARS = 1000;
/** Teto de trechos por célula — um `a` num JSON de 1 KB não vira 200 spans */
export const MAX_RANGES_PER_CELL = 20;

// Via construtor: o tsconfig raiz não fixa `target`, e o literal com a flag
// `u` não passaria no type-check.
const COMBINING_MARKS = new RegExp('\\p{M}', 'gu');
const COMBINING_MARK = new RegExp('^\\p{M}$', 'u');
const NON_ASCII = /[\u0080-\uffff]/;
const WORD_CHAR = new RegExp('[\\p{L}\\p{N}_]', 'u');

const stripAccents = (text: string) =>
  // ASCII puro (o caso comum em IDs, e-mails, números) não paga o NFD.
  NON_ASCII.test(text)
    ? text.normalize('NFD').replace(COMBINING_MARKS, '')
    : text;

interface Folder {
  fold: (text: string) => string;
}

const createFolder = (lower: boolean, accents: boolean): Folder => {
  if (lower && accents) return { fold: t => stripAccents(t).toLowerCase() };
  if (lower) return { fold: t => t.toLowerCase() };
  if (accents) return { fold: stripAccents };
  return { fold: t => t };
};

/**
 * Dobra o texto e devolve, para cada posição do texto dobrado, o offset do
 * original — necessário quando a dobra muda o comprimento (`é` decomposto,
 * `İ` minúsculo vira 2 unidades). No caso comum o comprimento é igual e o
 * mapa é a identidade.
 */
const foldWithMap = (
  text: string,
  folder: Folder,
): { folded: string; start: number[] | null; end: number[] | null } => {
  const folded = folder.fold(text);
  if (folded.length === text.length) return { folded, start: null, end: null };

  const start: number[] = [];
  const end: number[] = [];
  let out = '';
  let offset = 0;
  // Onde começam, no dobrado, os pedaços do último caractere-base: um acento
  // combinante logo depois estende o fim deles — o destaque não deixa o `´`
  // de fora do `e`.
  let baseFrom = 0;
  for (const char of text) {
    if (COMBINING_MARK.test(char)) {
      for (let i = baseFrom; i < end.length; i++) end[i] = offset + char.length;
    } else {
      baseFrom = end.length;
    }
    const piece = folder.fold(char);
    for (let i = 0; i < piece.length; i++) {
      start.push(offset);
      end.push(offset + char.length);
    }
    out += piece;
    offset += char.length;
  }
  return { folded: out, start, end };
};

const isWordBoundary = (text: string, index: number) =>
  index < 0 || index >= text.length || !WORD_CHAR.test(text[index]);

type Finder = (folded: string, onMatch: (s: number, e: number) => boolean) => void;

const buildFinder = (
  query: string,
  mode: GridSearchMode,
  caseSensitive: boolean,
): Finder | { error: string } => {
  switch (mode) {
    case 'exact':
      return (folded, onMatch) => {
        if (folded === query) onMatch(0, folded.length);
      };

    case 'prefix':
      return (folded, onMatch) => {
        if (folded.startsWith(query)) onMatch(0, query.length);
      };

    case 'contains':
    case 'word':
      return (folded, onMatch) => {
        let from = 0;
        while (from <= folded.length - query.length) {
          const at = folded.indexOf(query, from);
          if (at < 0) return;
          const end = at + query.length;
          const accept =
            mode === 'contains' ||
            (isWordBoundary(folded, at - 1) && isWordBoundary(folded, end));
          if (accept) {
            if (!onMatch(at, end)) return;
            from = end;
          } else {
            from = at + 1;
          }
        }
      };

    case 'regex': {
      let regex: RegExp;
      try {
        regex = new RegExp(query, caseSensitive ? 'gu' : 'giu');
      } catch (error) {
        return {
          error: error instanceof Error ? error.message : 'Regex inválida',
        };
      }
      return (folded, onMatch) => {
        regex.lastIndex = 0;
        let match: RegExpExecArray | null;
        while ((match = regex.exec(folded)) !== null) {
          // Casamento vazio (`^`, `a*`) não destaca nada e travaria o loop.
          if (match[0].length === 0) {
            regex.lastIndex++;
            continue;
          }
          if (!onMatch(match.index, match.index + match[0].length)) return;
        }
      };
    }
  }
};

/**
 * Compila a busca uma vez por (texto, opções). Query vazia → sem matcher (e
 * sem erro); regex inválida → sem matcher, com a mensagem para o campo.
 */
export const compileMatcher = (
  query: string,
  options: GridSearchOptions,
): CompiledSearch => {
  if (query.length === 0) return { matcher: null, error: null };

  const isRegex = options.mode === 'regex';
  // Na regex a caixa vai pela flag `i`: baixar o padrão quebraria `\D`, `\S`…
  const folder = createFolder(
    !options.caseSensitive && !isRegex,
    options.ignoreAccents,
  );
  const foldedQuery = folder.fold(query);

  const finder = buildFinder(foldedQuery, options.mode, options.caseSensitive);
  if ('error' in finder) return { matcher: null, error: finder.error };

  const test = (text: string) => {
    let found = false;
    finder(folder.fold(text), () => {
      found = true;
      return false;
    });
    return found;
  };

  const ranges = (text: string) => {
    const source =
      text.length > MAX_HIGHLIGHT_CHARS
        ? text.slice(0, MAX_HIGHLIGHT_CHARS)
        : text;
    const { folded, start, end } = foldWithMap(source, folder);
    const result: MatchRange[] = [];
    finder(folded, (s, e) => {
      result.push(start && end ? [start[s], end[e - 1]] : [s, e]);
      return result.length < MAX_RANGES_PER_CELL;
    });
    return result;
  };

  return { matcher: { test, ranges }, error: null };
};

/** Coluna-alvo da varredura: posição na grade e de onde ler o texto */
export interface SearchColumn {
  /** Índice entre as colunas desenhadas (ordem da grade) */
  visibleIndex: number;
  name: string;
}

/** Lê o texto que a célula mostra, ou `null` (NULL nunca casa) */
export type CellTextReader = (rowIndex: number, column: SearchColumn) => string | null;

/** Teto de resultados guardados; acima disso o contador mostra "10.000+" */
export const MAX_GRID_MATCHES = 10_000;

/**
 * Resultados em pares `[linha, colunaVisível]` num `Uint32Array` — ordenados
 * pela ordem de leitura da grade (linha, depois coluna), o que permite busca
 * binária na navegação.
 */
export interface GridMatches {
  data: Uint32Array;
  count: number;
  /** Parou no teto: existem mais resultados que os guardados */
  capped: boolean;
}

export const EMPTY_MATCHES: GridMatches = {
  data: new Uint32Array(0),
  count: 0,
  capped: false,
};

/** Estado de uma varredura em andamento, avançado em fatias por `scanSlice` */
export interface ScanState {
  row: number;
  data: Uint32Array;
  count: number;
  capped: boolean;
  done: boolean;
}

export const createScanState = (): ScanState => ({
  row: 0,
  data: new Uint32Array(256),
  count: 0,
  capped: false,
  done: false,
});

const pushMatch = (state: ScanState, row: number, column: number) => {
  const needed = (state.count + 1) * 2;
  if (needed > state.data.length) {
    const grown = new Uint32Array(Math.max(needed, state.data.length * 2));
    grown.set(state.data);
    state.data = grown;
  }
  state.data[state.count * 2] = row;
  state.data[state.count * 2 + 1] = column;
  state.count++;
};

/**
 * Avança a varredura até `deadline` (relógio de `now`). Processa linhas
 * inteiras e confere o relógio a cada 64 linhas — o custo da checagem some e
 * nenhuma fatia passa muito do orçamento.
 */
export const scanSlice = (
  state: ScanState,
  rowCount: number,
  columns: SearchColumn[],
  read: CellTextReader,
  matcher: GridMatcher,
  deadline: number,
  now: () => number = () => performance.now(),
) => {
  while (state.row < rowCount) {
    const row = state.row;
    for (const column of columns) {
      const text = read(row, column);
      if (text === null || !matcher.test(text)) continue;
      if (state.count >= MAX_GRID_MATCHES) {
        state.capped = true;
        state.done = true;
        return;
      }
      pushMatch(state, row, column.visibleIndex);
    }
    state.row++;
    if ((state.row & 63) === 0 && now() >= deadline) return;
  }
  state.done = true;
};

export const finishScan = (state: ScanState): GridMatches => ({
  data: state.data,
  count: state.count,
  capped: state.capped,
});

/**
 * Primeiro resultado em `(row, column)` ou depois dele, na ordem da grade —
 * `count` quando não há nenhum depois.
 */
export const lowerBoundMatch = (
  matches: GridMatches,
  row: number,
  column: number,
) => {
  let lo = 0;
  let hi = matches.count;
  while (lo < hi) {
    const mid = (lo + hi) >>> 1;
    const r = matches.data[mid * 2];
    const c = matches.data[mid * 2 + 1];
    if (r < row || (r === row && c < column)) lo = mid + 1;
    else hi = mid;
  }
  return lo;
};

export const matchAt = (matches: GridMatches, index: number) => ({
  rowIndex: matches.data[index * 2],
  visibleColumnIndex: matches.data[index * 2 + 1],
});

/** Devolve o controle ao navegador entre fatias (sem o clamp de 4 ms do setTimeout) */
export const yieldToMain = (): Promise<void> =>
  new Promise(resolve => {
    const channel = new MessageChannel();
    channel.port1.onmessage = () => {
      channel.port1.close();
      resolve();
    };
    channel.port2.postMessage(null);
  });
