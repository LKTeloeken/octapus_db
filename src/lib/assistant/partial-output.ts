import type { OutputKind } from '@/api/types/assistant.types';

/**
 * O que já dá para mostrar de uma resposta do modelo ainda chegando.
 *
 * O `delta` do turno traz o texto cru da saída estruturada — pedaços de um JSON
 * do `ModelOutput` (`{"kind":"query","query":"SELECT …","explanation":"Conta…`),
 * não prosa. Este extrator lê esse JSON incompleto e devolve os campos que já
 * apareceram, inclusive a string que ainda não fechou. O evento `output` traz a
 * versão final conferida pelo backend, que substitui esta.
 */
export interface PartialModelOutput {
  /** Só quando a string fechou e é um dos valores do contrato */
  kind: OutputKind | null;
  query: string | null;
  explanation: string | null;
  assumptions: string[];
  clarificationQuestion: string | null;
}

export const EMPTY_PARTIAL_OUTPUT: PartialModelOutput = {
  kind: null,
  query: null,
  explanation: null,
  assumptions: [],
  clarificationQuestion: null,
};

const OUTPUT_KINDS: readonly OutputKind[] = [
  'query',
  'code',
  'clarification',
  'ask_user',
  'structure_request',
  'answer',
];

const STRING_FIELDS = [
  'query',
  'explanation',
  'clarificationQuestion',
] as const;
type StringField = (typeof STRING_FIELDS)[number];

const isStringField = (key: string): key is StringField =>
  (STRING_FIELDS as readonly string[]).includes(key);

interface ScannedString {
  value: string;
  /** Índice logo depois da aspa final (ou o fim do texto, se não fechou) */
  end: number;
  closed: boolean;
}

const ESCAPES: Record<string, string> = {
  '"': '"',
  '\\': '\\',
  '/': '/',
  b: '\b',
  f: '\f',
  n: '\n',
  r: '\r',
  t: '\t',
};

/**
 * Lê uma string JSON a partir da aspa de abertura em `start`. Se o texto acaba
 * no meio dela, devolve o que veio — sem um escape cortado pela metade
 * (`\`, `\u00`), que só faz sentido inteiro.
 */
function scanString(raw: string, start: number): ScannedString {
  let value = '';
  let index = start + 1;

  while (index < raw.length) {
    const char = raw[index];

    if (char === '"') return { value, end: index + 1, closed: true };

    if (char !== '\\') {
      value += char;
      index += 1;
      continue;
    }

    const escape = raw[index + 1];
    if (escape === undefined) break;

    if (escape === 'u') {
      const hex = raw.slice(index + 2, index + 6);
      if (hex.length < 4) break;
      if (/^[0-9a-fA-F]{4}$/.test(hex)) {
        value += String.fromCharCode(Number.parseInt(hex, 16));
      }
      index += 6;
      continue;
    }

    value += ESCAPES[escape] ?? escape;
    index += 2;
  }

  return { value, end: raw.length, closed: false };
}

const skipSpace = (raw: string, index: number) => {
  while (index < raw.length && /\s/.test(raw[index])) index += 1;
  return index;
};

/**
 * Pula um valor que não interessa (número, literal, objeto, array): devolve o
 * índice depois dele, ou `null` se o texto acabou antes.
 */
function skipValue(raw: string, start: number): number | null {
  const first = raw[start];

  if (first === '"') {
    const scanned = scanString(raw, start);
    return scanned.closed ? scanned.end : null;
  }

  if (first === '{' || first === '[') {
    let depth = 0;
    let index = start;
    while (index < raw.length) {
      const char = raw[index];
      if (char === '"') {
        const scanned = scanString(raw, index);
        if (!scanned.closed) return null;
        index = scanned.end;
        continue;
      }
      if (char === '{' || char === '[') depth += 1;
      if (char === '}' || char === ']') {
        depth -= 1;
        if (depth === 0) return index + 1;
      }
      index += 1;
    }
    return null;
  }

  // Número ou literal: vai até o separador. Sem separador, pode estar cortado.
  let index = start;
  while (index < raw.length && !/[,}\]\s]/.test(raw[index])) index += 1;
  return index < raw.length ? index : null;
}

/** Os itens string de um array, o último possivelmente incompleto */
function scanStringArray(
  raw: string,
  start: number,
): { items: string[]; end: number | null } {
  const items: string[] = [];
  let index = start + 1;

  while (index < raw.length) {
    index = skipSpace(raw, index);
    const char = raw[index];
    if (char === undefined) break;
    if (char === ']') return { items, end: index + 1 };
    if (char === ',') {
      index += 1;
      continue;
    }
    if (char === '"') {
      const scanned = scanString(raw, index);
      if (scanned.value || scanned.closed) items.push(scanned.value);
      if (!scanned.closed) return { items, end: null };
      index = scanned.end;
      continue;
    }
    const next = skipValue(raw, index);
    if (next === null) return { items, end: null };
    index = next;
  }

  return { items, end: null };
}

/**
 * Lê o JSON parcial do `ModelOutput`. Tolera: texto cortado em qualquer ponto,
 * campos em qualquer ordem, uma cerca ```` ```json ```` antes do objeto e
 * valores de tipo inesperado (ignorados — a `query` objeto do Mongo fica para a
 * B8). Texto que não começa num objeto não rende nada.
 */
export function extractPartialOutput(raw: string): PartialModelOutput {
  const output: PartialModelOutput = {
    ...EMPTY_PARTIAL_OUTPUT,
    assumptions: [],
  };

  let index = skipSpace(raw, 0);
  if (raw.startsWith('```', index)) {
    const lineEnd = raw.indexOf('\n', index);
    if (lineEnd === -1) return output;
    index = skipSpace(raw, lineEnd + 1);
  }
  if (raw[index] !== '{') return output;
  index += 1;

  while (index < raw.length) {
    index = skipSpace(raw, index);
    const char = raw[index];
    if (char === undefined || char === '}') break;
    if (char === ',') {
      index += 1;
      continue;
    }
    if (char !== '"') break;

    const key = scanString(raw, index);
    if (!key.closed) break;
    index = skipSpace(raw, key.end);
    if (raw[index] !== ':') break;
    index = skipSpace(raw, index + 1);
    if (index >= raw.length) break;

    const valueStart = raw[index];

    if (
      valueStart === '"' &&
      (isStringField(key.value) || key.value === 'kind')
    ) {
      const scanned = scanString(raw, index);
      if (key.value === 'kind') {
        if (
          scanned.closed &&
          OUTPUT_KINDS.includes(scanned.value as OutputKind)
        ) {
          output.kind = scanned.value as OutputKind;
        }
      } else {
        output[key.value as StringField] = scanned.value;
      }
      if (!scanned.closed) break;
      index = scanned.end;
      continue;
    }

    if (valueStart === '[' && key.value === 'assumptions') {
      const scanned = scanStringArray(raw, index);
      output.assumptions = scanned.items;
      if (scanned.end === null) break;
      index = scanned.end;
      continue;
    }

    const next = skipValue(raw, index);
    if (next === null) break;
    index = next;
  }

  return output;
}
