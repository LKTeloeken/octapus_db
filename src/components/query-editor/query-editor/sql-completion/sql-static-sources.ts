import {
  ifNotIn,
  snippetCompletion,
  type Completion,
  type CompletionSource,
} from '@codemirror/autocomplete';
import type { SqlCompletionCategory } from './sql-clause-boost';

/**
 * Contextos onde sugestão de texto não faz sentido — mesma guarda que o
 * `keywordCompletionSource` do lang-sql já aplica sozinho.
 */
const SUPPRESSED_IN = [
  'QuotedIdentifier',
  'String',
  'LineComment',
  'BlockComment',
  '.',
];

/**
 * Combinações de duas palavras: a lista de keywords do dialeto só tem palavras soltas.
 * O casamento é feito pelo token da palavra atual (digitar `left` alcança `LEFT JOIN`).
 */
const POSTGRES_PHRASES = [
  'INNER JOIN',
  'LEFT JOIN',
  'RIGHT JOIN',
  'FULL JOIN',
  'CROSS JOIN',
  'GROUP BY',
  'ORDER BY',
  'UNION ALL',
  'IS NULL',
  'IS NOT NULL',
  'INSERT INTO',
  'DELETE FROM',
  'ON CONFLICT',
];

const POSTGRES_FUNCTIONS = [
  'COUNT',
  'SUM',
  'AVG',
  'MIN',
  'MAX',
  'NOW',
  'CURRENT_DATE',
  'CURRENT_TIME',
  'CURRENT_TIMESTAMP',
  'COALESCE',
  'NULLIF',
  'LOWER',
  'UPPER',
  'LENGTH',
  'SUBSTRING',
  'TRIM',
  'ROUND',
  'RANDOM',
  'ARRAY_AGG',
  'JSON_AGG',
  'JSONB_AGG',
  'TO_CHAR',
  'TO_DATE',
  'DATE_TRUNC',
  'EXTRACT',
];

const PHRASE_OPTIONS: Completion[] = POSTGRES_PHRASES.map(phrase => ({
  label: phrase,
  type: 'keyword',
}));

const FUNCTION_OPTIONS: Completion[] = POSTGRES_FUNCTIONS.map(fn => ({
  label: fn,
  type: 'function',
  detail: 'function',
  apply: `${fn}()`,
}));

const SNIPPET_OPTIONS: Completion[] = [
  snippetCompletion('SELECT ${columns} FROM ${table};', {
    label: 'select',
    detail: 'SELECT query',
    type: 'keyword',
  }),
  snippetCompletion('INSERT INTO ${table} (${columns}) VALUES (${values});', {
    label: 'insert',
    detail: 'INSERT query',
    type: 'keyword',
  }),
  snippetCompletion('UPDATE ${table} SET ${column} = ${value} WHERE ${id};', {
    label: 'update',
    detail: 'UPDATE query',
    type: 'keyword',
  }),
  snippetCompletion('DELETE FROM ${table} WHERE ${condition};', {
    label: 'delete',
    detail: 'DELETE query',
    type: 'keyword',
  }),
];

/**
 * Categoria de cada opção estática. Precisa ser um registro explícito: frases e snippets
 * carregam ambos `type: 'keyword'`, então inferir pelo `type` seria frágil.
 */
const OPTION_CATEGORY = new WeakMap<Completion, SqlCompletionCategory>();

for (const option of PHRASE_OPTIONS) OPTION_CATEGORY.set(option, 'keyword');
for (const option of FUNCTION_OPTIONS) OPTION_CATEGORY.set(option, 'function');
for (const option of SNIPPET_OPTIONS) OPTION_CATEGORY.set(option, 'snippet');

export function staticOptionCategory(
  option: Completion,
): SqlCompletionCategory {
  return OPTION_CATEGORY.get(option) ?? 'keyword';
}

const STATIC_OPTIONS: Completion[] = [
  ...PHRASE_OPTIONS,
  ...FUNCTION_OPTIONS,
  ...SNIPPET_OPTIONS,
];

/**
 * Frases, funções e snippets. O token é sempre uma palavra — labels com espaço são
 * alcançados pelo fuzzy matcher do CodeMirror, não pelo texto casado.
 */
export const sqlStaticSource: CompletionSource = ifNotIn(
  SUPPRESSED_IN,
  context => {
    const token = context.matchBefore(/\w*/);

    if (!token || (token.from === token.to && !context.explicit)) {
      return null;
    }

    return {
      from: token.from,
      options: STATIC_OPTIONS,
      validFor: /^\w*$/,
    };
  },
);
