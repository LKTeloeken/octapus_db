/**
 * Menções com `@` na pergunta ao assistente: `@public.users`, `@tenant_7`,
 * `@"Vendas"."Pedidos 2025"`. Elas dizem de quais schemas a estrutura vem.
 *
 * - **Postgres:** `@schema.tabela` cita uma relação; `@schema` sozinho cita o
 *   schema inteiro. O autocomplete sempre insere a forma qualificada, então
 *   `@nome` é lido como schema.
 * - **Sem schema** (SQLite, catálogo `FLAT_SCHEMA`): `@tabela` direto.
 *
 * O texto da menção vai junto na pergunta: é nome de estrutura, que pode sair
 * do app (RN-07), e reforça o casamento léxico que escolhe as tabelas (B4).
 * Além disso, toda relação ou schema citado na pergunta atual entra
 * obrigatoriamente no contexto (`conversationMentions`).
 */

import type { AssistantMention } from '@/api/types/assistant.types';

export interface Mention {
  /** Posição do `@` */
  from: number;
  /** Logo depois do último caractere da menção */
  to: number;
  /** `null` no catálogo sem schema */
  schema: string | null;
  /** `null` quando a menção é só do schema */
  table: string | null;
}

/** Identificador simples (letras com acento incluídas) */
const SIMPLE_IDENT = String.raw`[A-Za-z_\u00C0-\uFFFF][\w$\u00C0-\uFFFF]*`;

/** Identificador simples ou entre aspas (`""` escapa a aspa) */
const IDENT = String.raw`(?:"(?:[^"]|"")+"|${SIMPLE_IDENT})`;

/** O `@` só abre menção no começo do texto ou depois de espaço/pontuação */
const MENTION_SOURCE = String.raw`(^|[\s(\[{,;:!?])@(${IDENT})(?:\.(${IDENT}))?`;

/** Nome sem as aspas: `"Pedidos ""2025"""` → `Pedidos "2025"` */
export function unquoteIdent(ident: string): string {
  return ident.startsWith('"') && ident.endsWith('"') && ident.length >= 2
    ? ident.slice(1, -1).split('""').join('"')
    : ident;
}

/** O nome como vai na menção: entre aspas se não for identificador simples */
export function quoteIdent(name: string): string {
  return new RegExp(`^${SIMPLE_IDENT}$`).test(name)
    ? name
    : `"${name.split('"').join('""')}"`;
}

/** As menções do texto, na ordem em que aparecem */
export function parseMentions(text: string, flat: boolean): Mention[] {
  const mentions: Mention[] = [];
  const pattern = new RegExp(MENTION_SOURCE, 'g');

  for (let match = pattern.exec(text); match; match = pattern.exec(text)) {
    const [whole, lead, first, second] = match;
    const from = match.index + lead.length;
    const to = match.index + whole.length;

    if (flat) {
      // Sem schema, `@a.b` não existe: só o primeiro nome conta
      const end = second === undefined ? to : from + 1 + first.length;
      mentions.push({
        from,
        to: end,
        schema: null,
        table: unquoteIdent(first),
      });
      continue;
    }

    mentions.push({
      from,
      to,
      schema: unquoteIdent(first),
      table: second === undefined ? null : unquoteIdent(second),
    });
  }

  return mentions;
}

const pushUnique = (list: string[], value: string | null) => {
  if (value && !list.includes(value)) list.push(value);
};

/**
 * Os `schemas` do `assistant_turn_ask` (Postgres). Ordem, sem repetir:
 *
 * 1. os citados na pergunta atual, na ordem do texto — o **primeiro vai no
 *    `search_path`** da execução;
 * 2. os citados nas perguntas anteriores da conversa, da mais recente para a
 *    mais antiga (relidos do histórico do servidor: nada fica guardado aqui);
 * 3. o schema padrão do alvo (o do nó ou da aba de onde a conversa abriu), ou
 *    `public`.
 *
 * No catálogo sem schema devolve `[]`: o backend usa o schema sem nome.
 */
export function conversationSchemas(
  question: string,
  previousQuestions: string[],
  defaultSchema: string | null,
  flat: boolean,
): string[] {
  if (flat) return [];

  const schemas: string[] = [];
  for (const mention of parseMentions(question, false)) {
    pushUnique(schemas, mention.schema);
  }
  for (const previous of [...previousQuestions].reverse()) {
    for (const mention of parseMentions(previous, false)) {
      pushUnique(schemas, mention.schema);
    }
  }
  pushUnique(schemas, defaultSchema ?? 'public');
  return schemas;
}

const sameMention = (a: AssistantMention, b: AssistantMention) =>
  a.schema === b.schema && a.table === b.table;

const uniqueMentions = (texts: string[], flat: boolean) => {
  const list: AssistantMention[] = [];
  for (const text of texts) {
    for (const { schema, table } of parseMentions(text, flat)) {
      const mention = { schema, table };
      if (!list.some(other => sameMention(other, mention))) list.push(mention);
    }
  }
  return list;
};

/**
 * As `mentions` e `pastMentions` do `assistant_turn_ask`, sem repetir: as
 * citadas na pergunta atual entram obrigatoriamente no contexto da estrutura
 * (o backend confere cada uma no catálogo); as das perguntas anteriores, da
 * mais recente para a mais antiga, só ganham prioridade — e não repetem as
 * atuais.
 */
export function conversationMentions(
  question: string,
  previousQuestions: string[],
  flat: boolean,
): { mentions: AssistantMention[]; pastMentions: AssistantMention[] } {
  const mentions = uniqueMentions([question], flat);
  const pastMentions = uniqueMentions(
    [...previousQuestions].reverse(),
    flat,
  ).filter(past => !mentions.some(current => sameMention(current, past)));
  return { mentions, pastMentions };
}

/** Uma menção como o texto a escreve: `@schema.tabela`, `@schema`, `@tabela` */
export function mentionLabel({ schema, table }: AssistantMention): string {
  const parts = [schema, table].filter((part): part is string => part !== null);
  return `@${parts.map(quoteIdent).join('.')}`;
}

/** Destaque das menções no campo (o `@` e o nome, sem o caractere antes) */
export const MENTION_HIGHLIGHT = new RegExp(
  String.raw`(?<=^|[\s(\[{,;:!?])@${IDENT}(?:\.${IDENT})?`,
  'g',
);

/**
 * A menção sendo digitada logo antes do cursor: `@`, `@pub`, `@public.`,
 * `@public.us`, `@"Ven`. Devolve as partes já digitadas, ainda com aspas
 * abertas, ou `null` se o cursor não está numa menção.
 */
export interface MentionDraft {
  /** Texto desde o `@` */
  text: string;
  /** Primeiro nome (prefixo), sem aspas */
  first: string;
  /** Depois do ponto (prefixo), sem aspas; `null` = ainda sem ponto */
  second: string | null;
}

const PARTIAL_IDENT = String.raw`(?:"(?:[^"]|"")*"?|[\w$À-￿]*)`;
const DRAFT = new RegExp(
  String.raw`(?:^|[\s(\[{,;:!?])(@(${PARTIAL_IDENT})(?:\.(${PARTIAL_IDENT}))?)$`,
);

const unquotePartial = (ident: string) =>
  ident.startsWith('"')
    ? ident.slice(1).replace(/"$/, '').split('""').join('"')
    : ident;

export function mentionDraft(textBeforeCursor: string): MentionDraft | null {
  const match = DRAFT.exec(textBeforeCursor);
  if (!match) return null;
  const [, text, first, second] = match;
  return {
    text,
    first: unquotePartial(first),
    second: second === undefined ? null : unquotePartial(second),
  };
}
