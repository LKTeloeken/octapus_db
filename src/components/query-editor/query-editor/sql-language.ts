import { LanguageSupport, type LRLanguage } from '@codemirror/language';
import { PostgreSQL } from '@codemirror/lang-sql';
import { parseMixed, type Input, type SyntaxNodeRef } from '@lezer/common';

/** Tag de abertura de uma string `$tag$` (a vazia é o `$$`) */
const DOLLAR_TAG = /^\$(?:[A-Za-z_\u0080-￿][\w\u0080-￿]*)?\$/;

/** Uma tag nunca é maior que isso; poupa ler o corpo inteiro só para achá-la */
const MAX_TAG_LENGTH = 64;

/**
 * O miolo de uma string `$tag$ … $tag$`, ou null se o nó não é uma. Sem a tag de
 * fechamento (o usuário ainda está digitando o corpo) o miolo vai até o fim do nó —
 * é assim que o tokenizador do lang-sql também a lê.
 */
export function dollarQuotedBody(
  node: SyntaxNodeRef,
  input: Input,
): { from: number; to: number } | null {
  if (node.name !== 'String') return null;

  const head = input.read(
    node.from,
    Math.min(node.to, node.from + MAX_TAG_LENGTH),
  );
  const tag = DOLLAR_TAG.exec(head)?.[0];
  if (!tag) return null;

  const from = node.from + tag.length;
  const closed =
    node.to - tag.length >= from &&
    input.read(node.to - tag.length, node.to) === tag;
  const to = closed ? node.to - tag.length : node.to;

  return to > from ? { from, to } : null;
}

/**
 * PostgreSQL com o corpo das strings `$$` analisado de novo como SQL. O dialeto do
 * lang-sql lê `$$ … $$` como um token `String` só — o corpo de um `CREATE FUNCTION`
 * ou de um `DO` ficava todo da cor de string. O `parseMixed` monta uma árvore SQL
 * sobre o miolo (overlay), então keywords, números e comentários lá dentro ganham o
 * highlight normal, e a sugestão de código também passa a valer ali dentro. As tags
 * continuam com a cor de string; strings `'…'` não mudam. `$x$` aninhado dentro do
 * corpo também entra, pela mesma regra (o parser do overlay é este mesmo).
 *
 * Efeito colateral aceito: um literal `select $$texto$$` também é colorido como SQL.
 */
export const postgresLanguage: LRLanguage = PostgreSQL.language.configure({
  wrap: parseMixed((node, input) => {
    const body = dollarQuotedBody(node, input);
    return body ? { parser: postgresLanguage.parser, overlay: [body] } : null;
  }),
});

/**
 * O suporte de linguagem do editor SQL. Sem as fontes de sugestão do `sql()`: o
 * editor monta as próprias (`completionExtension`, com `override`).
 */
export const postgresSupport = (): LanguageSupport =>
  new LanguageSupport(postgresLanguage);
