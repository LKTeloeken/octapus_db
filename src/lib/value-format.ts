/**
 * Formatação de valores de célula para o painel de valor: JSON, XML, HTML e a
 * visão binária. Tudo puro e síncrono — recebe o texto cru da célula (toda
 * célula do backend é `string | null`) e devolve texto.
 *
 * O JSON é reindentado por tokens, sem `JSON.parse` → `JSON.stringify`: o
 * caminho ida-e-volta arredondaria inteiros acima de 2^53 e normalizaria os
 * escapes das strings, e o texto formatado pode acabar gravado no banco.
 */

export type ValueFormat = 'binary' | 'html' | 'json' | 'text' | 'xml';

export type ValueEncoding =
  | 'utf-8'
  | 'iso-8859-1'
  | 'us-ascii'
  | 'utf-16le'
  | 'utf-16be';

export const VALUE_ENCODINGS: { value: ValueEncoding; label: string }[] = [
  { value: 'utf-8', label: 'UTF-8' },
  { value: 'iso-8859-1', label: 'ISO-8859-1 (Latin-1)' },
  { value: 'us-ascii', label: 'US-ASCII' },
  { value: 'utf-16le', label: 'UTF-16LE' },
  { value: 'utf-16be', label: 'UTF-16BE' },
];

// ── JSON ────────────────────────────────────────────────────────────────────

export function isValidJson(text: string): boolean {
  try {
    JSON.parse(text);
    return true;
  } catch {
    return false;
  }
}

/**
 * Reescreve o espaçamento de um JSON **já validado**, copiando cada token como
 * está. `indent === null` compacta.
 */
function reformatJson(text: string, indent: string | null): string {
  let out = '';
  let depth = 0;
  let inString = false;
  const newline = () => (indent === null ? '' : '\n' + indent.repeat(depth));

  for (let i = 0; i < text.length; i++) {
    const ch = text[i];

    if (inString) {
      out += ch;
      if (ch === '\\') out += text[++i] ?? '';
      else if (ch === '"') inString = false;
      continue;
    }

    switch (ch) {
      case '"':
        inString = true;
        out += ch;
        break;
      case '{':
      case '[': {
        // Contêiner vazio fica numa linha só: {} / [].
        const close = ch === '{' ? '}' : ']';
        let j = i + 1;
        while (j < text.length && /\s/.test(text[j])) j++;
        if (text[j] === close) {
          out += ch + close;
          i = j;
          break;
        }
        depth++;
        out += ch + newline();
        break;
      }
      case '}':
      case ']':
        depth--;
        out += newline() + ch;
        break;
      case ',':
        out += ',' + newline();
        break;
      case ':':
        out += indent === null ? ':' : ': ';
        break;
      case ' ':
      case '\t':
      case '\n':
      case '\r':
        break;
      default:
        out += ch;
    }
  }

  return out;
}

/** JSON indentado com 2 espaços, ou `null` se o texto não for JSON válido. */
export function prettyJson(text: string): string | null {
  return isValidJson(text) ? reformatJson(text, '  ') : null;
}

/** JSON sem espaços supérfluos, ou `null` se o texto não for JSON válido. */
export function minifyJson(text: string): string | null {
  return isValidJson(text) ? reformatJson(text, null) : null;
}

/**
 * Lê um array JSON de valores simples preservando o texto de cada número e
 * booleano (o `JSON.parse` perderia a precisão de um bigint). Devolve `null`
 * se o texto não for um array JSON plano — objetos e arrays aninhados ficam de
 * fora.
 */
export function parseJsonList(text: string): (string | null)[] | null {
  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch {
    return null;
  }
  if (!Array.isArray(parsed)) return null;
  if (parsed.some(item => typeof item === 'object' && item !== null)) {
    return null;
  }

  // Estrutura já validada: basta recortar os tokens do primeiro nível.
  const result: (string | null)[] = [];
  let i = text.indexOf('[') + 1;
  const skipSpaces = () => {
    while (i < text.length && /\s/.test(text[i])) i++;
  };

  skipSpaces();
  if (text[i] === ']') return result;

  while (i < text.length) {
    skipSpaces();
    if (text[i] === '"') {
      let j = i + 1;
      while (j < text.length && text[j] !== '"') j += text[j] === '\\' ? 2 : 1;
      result.push(JSON.parse(text.slice(i, j + 1)) as string);
      i = j + 1;
    } else {
      let j = i;
      while (j < text.length && !/[\s,\]]/.test(text[j])) j++;
      const token = text.slice(i, j);
      result.push(token === 'null' ? null : token);
      i = j;
    }
    skipSpaces();
    if (text[i] !== ',') break;
    i++;
  }

  return result;
}

// ── XML / HTML ──────────────────────────────────────────────────────────────
// O parse usa DOMParser e <template>: os dois produzem documentos inertes —
// nenhum script roda e nenhum recurso (img, link) é carregado.

const HTML_VOID = new Set([
  'area',
  'base',
  'br',
  'col',
  'embed',
  'hr',
  'img',
  'input',
  'link',
  'meta',
  'source',
  'track',
  'wbr',
]);

/** Conteúdo que não pode ser reindentado sem mudar o que ele significa. */
const HTML_RAW = new Set(['pre', 'textarea', 'script', 'style', 'template']);

const escapeText = (text: string) =>
  text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

const escapeAttribute = (text: string) =>
  text.replace(/&/g, '&amp;').replace(/"/g, '&quot;').replace(/</g, '&lt;');

const isBlankText = (node: Node) =>
  node.nodeType === Node.TEXT_NODE && !node.textContent?.trim();

function startTag(element: Element, name: string, selfClose: boolean): string {
  const attributes = Array.from(element.attributes)
    .map(attr => ` ${attr.name}="${escapeAttribute(attr.value)}"`)
    .join('');
  return `<${name}${attributes}${selfClose ? '/>' : '>'}`;
}

function doctypeTag(doctype: DocumentType): string {
  let tag = `<!DOCTYPE ${doctype.name}`;
  if (doctype.publicId) tag += ` PUBLIC "${doctype.publicId}"`;
  if (doctype.systemId) {
    tag += `${doctype.publicId ? '' : ' SYSTEM'} "${doctype.systemId}"`;
  }
  return `${tag}>`;
}

function formatElement(
  element: Element,
  depth: number,
  html: boolean,
  lines: string[],
) {
  const pad = '  '.repeat(depth);
  // HTML: `tagName` vem em maiúsculas; XML: `tagName` preserva caixa e prefixo.
  const name = html ? element.localName : element.tagName;

  if (html && HTML_VOID.has(name)) {
    lines.push(pad + startTag(element, name, false));
    return;
  }
  if (html && HTML_RAW.has(name)) {
    lines.push(pad + element.outerHTML);
    return;
  }

  const children = Array.from(element.childNodes).filter(
    child => !isBlankText(child),
  );

  if (children.length === 0) {
    lines.push(
      pad +
        (html
          ? `${startTag(element, name, false)}</${name}>`
          : startTag(element, name, true)),
    );
    return;
  }

  // Só texto dentro: fica numa linha, `<nome>texto</nome>`.
  if (children.length === 1 && children[0].nodeType === Node.TEXT_NODE) {
    const text = escapeText(children[0].textContent?.trim() ?? '');
    lines.push(`${pad}${startTag(element, name, false)}${text}</${name}>`);
    return;
  }

  lines.push(pad + startTag(element, name, false));
  formatNodes(children, depth + 1, html, lines);
  lines.push(`${pad}</${name}>`);
}

function formatNodes(
  nodes: ChildNode[],
  depth: number,
  html: boolean,
  lines: string[],
) {
  const pad = '  '.repeat(depth);

  for (const node of nodes) {
    switch (node.nodeType) {
      case Node.ELEMENT_NODE:
        formatElement(node as Element, depth, html, lines);
        break;
      case Node.TEXT_NODE: {
        const text = node.textContent?.trim();
        if (text) lines.push(pad + escapeText(text));
        break;
      }
      case Node.CDATA_SECTION_NODE:
        lines.push(`${pad}<![CDATA[${node.textContent ?? ''}]]>`);
        break;
      case Node.COMMENT_NODE:
        lines.push(`${pad}<!--${node.textContent ?? ''}-->`);
        break;
      case Node.PROCESSING_INSTRUCTION_NODE: {
        const pi = node as ProcessingInstruction;
        lines.push(`${pad}<?${pi.target} ${pi.data}?>`);
        break;
      }
      case Node.DOCUMENT_TYPE_NODE:
        lines.push(pad + doctypeTag(node as DocumentType));
        break;
    }
  }
}

/** A declaração `<?xml …?>` não vira nó no DOM — guardamos o texto dela. */
function parseXml(text: string): { doc: Document; declaration: string } | null {
  const doc = new DOMParser().parseFromString(text, 'application/xml');
  if (doc.getElementsByTagName('parsererror').length > 0) return null;
  const declaration = /^\s*(<\?xml[^>]*\?>)/.exec(text)?.[1] ?? '';
  return { doc, declaration };
}

export function isValidXml(text: string): boolean {
  return parseXml(text) !== null;
}

export function prettyXml(text: string): string | null {
  const parsed = parseXml(text);
  if (!parsed) return null;

  const lines = parsed.declaration ? [parsed.declaration] : [];
  formatNodes(Array.from(parsed.doc.childNodes), 0, false, lines);
  return lines.join('\n');
}

function removeBlankTextNodes(node: Node) {
  for (const child of Array.from(node.childNodes)) {
    if (isBlankText(child)) node.removeChild(child);
    else removeBlankTextNodes(child);
  }
}

/** Tira só o espaço entre tags; texto misto (`a <b>b</b> c`) fica intacto. */
export function minifyXml(text: string): string | null {
  const parsed = parseXml(text);
  if (!parsed) return null;

  removeBlankTextNodes(parsed.doc);
  const body = new XMLSerializer()
    .serializeToString(parsed.doc)
    .replace(/^<\?xml[^>]*\?>/, '');
  return parsed.declaration + body;
}

/** Documento completo vai pelo DOMParser; fragmento, por <template> (o parser
 *  de documento embrulharia o fragmento em html/head/body). */
export function prettyHtml(text: string): string | null {
  try {
    const lines: string[] = [];
    if (/^\s*(<!doctype|<html[\s>])/i.test(text)) {
      const doc = new DOMParser().parseFromString(text, 'text/html');
      formatNodes(Array.from(doc.childNodes), 0, true, lines);
    } else {
      const template = document.createElement('template');
      template.innerHTML = text;
      formatNodes(Array.from(template.content.childNodes), 0, true, lines);
    }
    return lines.join('\n');
  } catch {
    return null;
  }
}

const HTML_HINT =
  /^<(!doctype\s+html|html|head|body|div|span|p|a|table|ul|ol|li|h[1-6]|br|img|section|article|header|footer|nav|main|strong|em|b|i)[\s>/]/i;

/** Adivinha o formato pelo conteúdo — o que o DBeaver chama de content type. */
export function sniffFormat(text: string): ValueFormat {
  const trimmed = text.trimStart();
  if (
    (trimmed.startsWith('{') || trimmed.startsWith('[')) &&
    isValidJson(text)
  ) {
    return 'json';
  }
  if (trimmed.startsWith('<')) {
    if (HTML_HINT.test(trimmed)) return 'html';
    if (isValidXml(text)) return 'xml';
  }
  return 'text';
}

// ── Binário ─────────────────────────────────────────────────────────────────

/** Acima disso o dump é cortado: gerar e desenhar megabytes de hex trava a UI. */
const MAX_BINARY_BYTES = 1024 * 1024;

/** Bytes do texto na codificação escolhida; fora do repertório vira `?`. */
export function encodeText(text: string, encoding: ValueEncoding): Uint8Array {
  switch (encoding) {
    case 'utf-8':
      return new TextEncoder().encode(text);

    case 'utf-16le':
    case 'utf-16be': {
      const bytes = new Uint8Array(text.length * 2);
      for (let i = 0; i < text.length; i++) {
        const code = text.charCodeAt(i);
        const high = code >> 8;
        const low = code & 0xff;
        bytes[i * 2] = encoding === 'utf-16le' ? low : high;
        bytes[i * 2 + 1] = encoding === 'utf-16le' ? high : low;
      }
      return bytes;
    }

    case 'iso-8859-1':
    case 'us-ascii': {
      const max = encoding === 'us-ascii' ? 0x7f : 0xff;
      const bytes: number[] = [];
      for (const char of text) {
        const code = char.codePointAt(0) ?? 0x3f;
        bytes.push(code <= max ? code : 0x3f);
      }
      return Uint8Array.from(bytes);
    }
  }
}

/** Dump clássico: offset, 16 bytes em hex e a coluna ASCII. */
export function hexDump(bytes: Uint8Array): string {
  const total = Math.min(bytes.length, MAX_BINARY_BYTES);
  const lines: string[] = [];

  for (let offset = 0; offset < total; offset += 16) {
    let hex = '';
    let ascii = '';
    for (let k = 0; k < 16; k++) {
      if (k === 8) hex += ' ';
      const index = offset + k;
      if (index < total) {
        const byte = bytes[index];
        hex += byte.toString(16).padStart(2, '0') + ' ';
        ascii += byte >= 0x20 && byte < 0x7f ? String.fromCharCode(byte) : '.';
      } else {
        hex += '   ';
      }
    }
    lines.push(`${offset.toString(16).padStart(8, '0')}  ${hex} |${ascii}|`);
  }

  if (bytes.length > total) {
    lines.push(`… ${bytes.length - total} bytes omitidos`);
  }
  return lines.join('\n');
}

export function formatBytes(size: number): string {
  if (size < 1024) return `${size} B`;
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)} KB`;
  return `${(size / (1024 * 1024)).toFixed(1)} MB`;
}
