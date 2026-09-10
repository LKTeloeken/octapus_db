/**
 * Arrays do Postgres **não são JSON**. O servidor renderiza a coluna com
 * `::text` (`array_out`), o que produz o literal de array — `{a,b}`,
 * `{"com, vírgula",NULL}`, `{}` — e o caminho de escrita devolve esse mesmo
 * literal no cast `$N::text::<tipo>[]`. Este módulo converte o literal de/para
 * uma lista de elementos para o editor da grade.
 *
 * Regras cobertas (as do `array_in`/`array_out`):
 * - `NULL` sem aspas é o elemento nulo; `"NULL"` é a string "NULL";
 * - elementos entre aspas duplas escapam `"` e `\` com barra invertida;
 * - espaço fora das aspas, antes/depois do elemento, é ignorado;
 * - `{}` é o array vazio.
 *
 * Multidimensionais (`{{1,2},{3,4}}`) e o prefixo de dimensões (`[1:3]={...}`)
 * não viram lista: `parsePgArray` devolve `null` e o editor cai no modo texto.
 */

/** Um elemento do array: o texto cru, ou `null` para o `NULL` do Postgres. */
export type PgArrayElement = string | null;

/** Caracteres que obrigam o elemento a sair entre aspas. */
const NEEDS_QUOTES = /[{},"\\\s]/;

/** `_varchar` (typname de `varchar[]`) e `text[]` são arrays; `varchar` não. */
export function isPgArrayType(typeName: string): boolean {
  const t = typeName.trim().toLowerCase();
  return t.startsWith('_') || t.endsWith('[]');
}

/** Rótulo amigável: `_varchar` → `varchar[]`. */
export function pgArrayTypeLabel(typeName: string): string {
  const t = typeName.trim();
  return t.startsWith('_') ? `${t.slice(1)}[]` : t;
}

/**
 * Lê o literal de array do Postgres. Devolve `null` quando não dá para
 * representar como lista simples (multidimensional, prefixo de dimensões ou
 * literal malformado) — nesse caso o editor mantém o texto cru.
 */
export function parsePgArray(literal: string): PgArrayElement[] | null {
  const input = literal.trim();
  if (!input.startsWith('{') || !input.endsWith('}') || input.length < 2) {
    return null;
  }

  const body = input.slice(1, -1);
  if (body.trim() === '') return [];

  const elements: PgArrayElement[] = [];
  let value = '';
  /** Espaço fora de aspas ainda não confirmado (só entra se vier mais texto). */
  let pending = '';
  /** O elemento passou por aspas ou barra invertida → é texto, nunca `NULL`. */
  let isLiteralText = false;
  let inQuotes = false;

  const pushElement = (): boolean => {
    if (!isLiteralText) {
      // `{a,}` é literal malformado no Postgres — não inventamos string vazia.
      if (value === '') return false;
      if (value.toUpperCase() === 'NULL') {
        elements.push(null);
        return true;
      }
    }
    elements.push(value);
    return true;
  };

  let i = 0;
  while (i < body.length) {
    const ch = body[i];

    if (inQuotes) {
      if (ch === '\\') {
        if (i + 1 >= body.length) return null;
        value += body[i + 1];
        i += 2;
        continue;
      }
      if (ch === '"') {
        inQuotes = false;
        i += 1;
        continue;
      }
      value += ch;
      i += 1;
      continue;
    }

    if (ch === '"') {
      value += pending;
      pending = '';
      isLiteralText = true;
      inQuotes = true;
      i += 1;
      continue;
    }

    if (ch === '\\') {
      if (i + 1 >= body.length) return null;
      value += pending + body[i + 1];
      pending = '';
      isLiteralText = true;
      i += 2;
      continue;
    }

    // Chave fora de aspas = outra dimensão; sai do modo lista.
    if (ch === '{' || ch === '}') return null;

    if (ch === ',') {
      if (!pushElement()) return null;
      value = '';
      pending = '';
      isLiteralText = false;
      i += 1;
      continue;
    }

    if (/\s/.test(ch)) {
      // Espaço à esquerda some; no meio do elemento fica pendente até que
      // apareça mais conteúdo (assim o espaço à direita também some).
      if (value !== '' || isLiteralText) pending += ch;
      i += 1;
      continue;
    }

    value += pending + ch;
    pending = '';
    i += 1;
  }

  if (inQuotes) return null;
  if (!pushElement()) return null;

  return elements;
}

/** Escreve o literal de array aceito pelo `$N::text::<tipo>[]`. */
export function formatPgArray(elements: PgArrayElement[]): string {
  return `{${elements.map(formatElement).join(',')}}`;
}

function formatElement(element: PgArrayElement): string {
  if (element === null) return 'NULL';

  const needsQuotes =
    element === '' ||
    NEEDS_QUOTES.test(element) ||
    element.toUpperCase() === 'NULL';

  if (!needsQuotes) return element;

  return `"${element.replace(/\\/g, '\\\\').replace(/"/g, '\\"')}"`;
}
