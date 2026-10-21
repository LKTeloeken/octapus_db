/**
 * A resposta em texto do assistente (`kind: answer`) em blocos para a tela:
 * parágrafos (com as quebras de linha) e blocos de código cercados por
 * ```` ``` ````. Dentro do parágrafo, trechos entre crases simples viram código
 * em linha. Nada vira HTML: a tela monta os elementos com o texto como texto.
 *
 * Tolera a resposta ainda chegando: uma cerca aberta sem fechar é código até
 * o fim.
 */

export type AnswerSpan = { code: boolean; text: string };

export type AnswerBlock =
  | { type: 'paragraph'; spans: AnswerSpan[] }
  | { type: 'code'; language: string | null; code: string };

const FENCE = /^ {0,3}```\s*([\w+#.-]*)\s*$/;

/** Os trechos de um parágrafo: texto e `código em linha` */
export function splitInline(text: string): AnswerSpan[] {
  const spans: AnswerSpan[] = [];
  const pattern = /`([^`\n]+)`/g;
  let last = 0;
  for (const match of Array.from(text.matchAll(pattern))) {
    const start = match.index ?? 0;
    if (start > last)
      spans.push({ code: false, text: text.slice(last, start) });
    spans.push({ code: true, text: match[1] });
    last = start + match[0].length;
  }
  if (last < text.length) spans.push({ code: false, text: text.slice(last) });
  return spans;
}

export function splitAnswer(text: string): AnswerBlock[] {
  const blocks: AnswerBlock[] = [];
  let paragraph: string[] = [];
  let fence: { language: string | null; lines: string[] } | null = null;

  const closeParagraph = () => {
    const joined = paragraph.join('\n').trim();
    if (joined) blocks.push({ type: 'paragraph', spans: splitInline(joined) });
    paragraph = [];
  };

  for (const line of text.replace(/\r\n?/g, '\n').split('\n')) {
    const marker = FENCE.exec(line);
    if (fence) {
      if (marker && !marker[1]) {
        blocks.push({
          type: 'code',
          language: fence.language,
          code: fence.lines.join('\n'),
        });
        fence = null;
      } else {
        fence.lines.push(line);
      }
      continue;
    }
    if (marker) {
      closeParagraph();
      fence = { language: marker[1] || null, lines: [] };
      continue;
    }
    if (line.trim() === '') closeParagraph();
    else paragraph.push(line);
  }

  if (fence) {
    blocks.push({
      type: 'code',
      language: fence.language,
      code: fence.lines.join('\n'),
    });
  }
  closeParagraph();
  return blocks;
}
