import type {
  Completion,
  CompletionContext,
  CompletionResult,
  CompletionSource,
} from '@codemirror/autocomplete';
import { startCompletion } from '@codemirror/autocomplete';
import { FLAT_SCHEMA, type CatalogNode } from '@/api/types/catalog.types';
import { mentionDraft, quoteIdent } from './mentions';

/**
 * Busca uma fatia do catálogo por prefixo (`catalog_complete`): `schema`
 * `null` = schemas; senão as relações dele. Nunca a estrutura inteira — num
 * database com 5.000 schemas, o `@` pede só os que casam com o prefixo.
 */
export type CompleteFetch = (
  schema: string | null,
  prefix: string,
) => Promise<CatalogNode[]>;

export interface MentionSourceOptions {
  fetch: CompleteFetch;
  /** Catálogo sem schema (SQLite): `@tabela` direto */
  flat: boolean;
  /** Schema cujas tabelas aparecem já no `@` (o padrão da conversa) */
  defaultSchema: string | null;
}

const RELATION_DETAIL: Partial<Record<CatalogNode['kind'], string>> = {
  table: 'tabela',
  view: 'view',
  materializedView: 'view materializada',
  foreign: 'tabela externa',
  partitioned: 'tabela particionada',
};

const relationOption = (
  node: CatalogNode,
  schema: string | null,
): Completion => {
  const text = `@${schema === null ? '' : `${quoteIdent(schema)}.`}${quoteIdent(node.name)}`;
  return {
    label: text,
    apply: `${text} `,
    type: 'class',
    detail: RELATION_DETAIL[node.kind] ?? node.kind,
    boost: schema === null ? 0 : 1,
  };
};

const schemaOption = (node: CatalogNode): Completion => ({
  label: `@${quoteIdent(node.name)}.`,
  type: 'namespace',
  detail: 'schema',
  // Escolher o schema já abre as tabelas dele
  apply: (view, _completion, from, to) => {
    const insert = `@${quoteIdent(node.name)}.`;
    view.dispatch({
      changes: { from, to, insert },
      selection: { anchor: from + insert.length },
    });
    startCompletion(view);
  },
});

/** As opções para a menção sendo digitada (sem CodeMirror: testável) */
export async function mentionOptions(
  textBeforeCursor: string,
  { fetch, flat, defaultSchema }: MentionSourceOptions,
): Promise<{ text: string; options: Completion[] } | null> {
  const draft = mentionDraft(textBeforeCursor);
  if (!draft) return null;

  if (flat) {
    if (draft.second !== null) return null;
    const nodes = await fetch(FLAT_SCHEMA, draft.first);
    return {
      text: draft.text,
      options: nodes.map(node => relationOption(node, null)),
    };
  }

  if (draft.second !== null) {
    const nodes = await fetch(draft.first, draft.second);
    return {
      text: draft.text,
      options: nodes.map(node => relationOption(node, draft.first)),
    };
  }

  const fallback = defaultSchema ?? 'public';
  const [schemas, relations] = await Promise.all([
    fetch(null, draft.first),
    fetch(fallback, draft.first),
  ]);
  return {
    text: draft.text,
    options: [
      ...schemas.map(schemaOption),
      ...relations.map(node => relationOption(node, fallback)),
    ],
  };
}

/** A fonte do autocomplete do campo da pergunta */
export function createMentionSource(
  options: () => MentionSourceOptions | null,
): CompletionSource {
  return async (
    context: CompletionContext,
  ): Promise<CompletionResult | null> => {
    const current = options();
    if (!current) return null;
    const line = context.state.doc.lineAt(context.pos);
    const before = line.text.slice(0, context.pos - line.from);
    // Só abre sozinho logo depois do `@`; no meio da palavra, pelo atalho
    if (!context.explicit && !/@[^\s]*$/.test(before)) return null;

    let found: Awaited<ReturnType<typeof mentionOptions>>;
    try {
      found = await mentionOptions(before, current);
    } catch {
      return null;
    }
    if (!found || context.aborted) return null;

    return {
      from: context.pos - found.text.length,
      options: found.options,
      // O backend já filtrou pelo prefixo; cada tecla pede de novo
      filter: false,
    };
  };
}
