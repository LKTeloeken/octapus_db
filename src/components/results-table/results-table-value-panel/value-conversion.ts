import { formatPgArray, parsePgArray } from '@/lib/pg-array';
import {
  encodeText,
  hexDump,
  isValidJson,
  isValidXml,
  minifyJson,
  minifyXml,
  parseJsonList,
  prettyHtml,
  prettyJson,
  prettyXml,
  sniffFormat,
  type ValueEncoding,
  type ValueFormat,
} from '@/lib/value-format';
import { resolveCellEditor } from '../results-table-cell/use-resolve-cell-editor';

/**
 * Categoria da coluna para o painel de valor. Só `json`, `array` e `text`
 * ganham o menu de formatação; `scalar` (número, data, uuid, booleano…) é
 * editado como texto simples.
 */
export type ValueKind = 'json' | 'array' | 'text' | 'scalar';

const TEXT_TYPES = new Set([
  'text',
  'varchar',
  'character varying',
  'char',
  'character',
  'bpchar',
  'name',
  'citext',
  'xml',
  'clob',
  'nvarchar',
  'nchar',
  'tinytext',
  'mediumtext',
  'longtext',
  // Mongo: `string`, e `mixed` quando o campo varia de tipo entre documentos
  // (o conteúdo decide o formato).
  'string',
  'mixed',
]);

/** Mongo: documento e array aninhados chegam como Extended JSON relaxado —
 *  o `array` daqui não é o literal `{a,b}` do Postgres. */
const MONGO_JSON_TYPES = new Set(['object', 'array']);

/** `varchar(255)` → `varchar`; `character(2)` → `character`. */
const normalizeType = (typeName: string) =>
  typeName
    .toLowerCase()
    .trim()
    .replace(/\s*\(.*\)$/, '');

export function resolveValueKind(typeName: string): ValueKind {
  const type = normalizeType(typeName);
  if (MONGO_JSON_TYPES.has(type)) return 'json';

  const editor = resolveCellEditor(type);
  if (editor === 'json') return 'json';
  if (editor === 'array') return 'array';
  if (TEXT_TYPES.has(type)) return 'text';
  return 'scalar';
}

/** Formato inicial de uma célula: pelo tipo da coluna e, em texto, pelo conteúdo. */
export function detectValueFormat(
  value: string | null,
  kind: ValueKind,
  typeName: string,
): ValueFormat {
  switch (kind) {
    case 'json':
      return 'json';
    case 'array':
      // Multidimensional não vira lista — fica no literal cru.
      return value === null || parsePgArray(value) !== null ? 'json' : 'text';
    case 'text':
      if (normalizeType(typeName) === 'xml') return 'xml';
      return value === null ? 'text' : sniffFormat(value);
    case 'scalar':
      return 'text';
  }
}

export interface EditorTextOptions {
  autoFormat: boolean;
  encoding: ValueEncoding;
}

/** Texto que o editor mostra para o valor da célula no formato escolhido. */
export function toEditorText(
  value: string | null,
  format: ValueFormat,
  kind: ValueKind,
  { autoFormat, encoding }: EditorTextOptions,
): string {
  if (value === null) return '';

  switch (format) {
    case 'binary':
      return hexDump(encodeText(value, encoding));
    case 'json':
      // Array do Postgres na visão JSON: a lista de elementos (sempre texto,
      // como o `array_out` devolve), editável como array JSON.
      if (kind === 'array') {
        const elements = parsePgArray(value);
        if (!elements) return value;
        return JSON.stringify(elements, null, autoFormat ? 2 : undefined);
      }
      return autoFormat ? (prettyJson(value) ?? value) : value;
    case 'xml':
      return autoFormat ? (prettyXml(value) ?? value) : value;
    case 'html':
      return autoFormat ? (prettyHtml(value) ?? value) : value;
    case 'text':
      return value;
  }
}

/**
 * Valor a gravar na célula a partir do texto do editor. `error` bloqueia
 * (o valor não seria aceito pelo banco); `warning` só avisa.
 */
export type StoredValueResult =
  | { ok: true; value: string; warning?: string }
  | { ok: false; error: string };

export function toStoredValue(
  text: string,
  format: ValueFormat,
  kind: ValueKind,
  saveCompact: boolean,
): StoredValueResult {
  if (kind === 'array' && format === 'json') {
    const elements = parseJsonList(text);
    if (!elements) {
      return {
        ok: false,
        error: 'Lista inválida — use um array JSON de valores simples',
      };
    }
    return { ok: true, value: formatPgArray(elements) };
  }

  // Coluna JSON só aceita JSON, qualquer que seja a visão escolhida.
  if (kind === 'json' && !isValidJson(text)) {
    return { ok: false, error: 'JSON inválido — alteração não aplicada' };
  }

  if (format === 'json') {
    if (!isValidJson(text)) {
      return { ok: true, value: text, warning: 'Conteúdo não é JSON válido' };
    }
    return { ok: true, value: saveCompact ? (minifyJson(text) ?? text) : text };
  }

  if (format === 'xml') {
    if (!isValidXml(text)) {
      return { ok: true, value: text, warning: 'Conteúdo não é XML válido' };
    }
    return { ok: true, value: saveCompact ? (minifyXml(text) ?? text) : text };
  }

  return { ok: true, value: text };
}
