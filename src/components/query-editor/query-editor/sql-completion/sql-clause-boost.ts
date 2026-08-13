import type {
  Completion,
  CompletionContext,
  CompletionResult,
  CompletionSource,
} from '@codemirror/autocomplete';
import { getStatementContextAt } from './sql-statement-context';
import type { SqlClause } from './sql-completion.types';

export type SqlCompletionCategory =
  | 'column'
  | 'table'
  | 'keyword'
  | 'function'
  | 'snippet';

/** `null` = esta categoria não deve nem ser oferecida nesta cláusula. */
export type ClauseBoost = number | null;

/**
 * Peso de cada categoria por cláusula.
 *
 * Com padrão vazio (o atalho de sugestão explícita) o `FuzzyMatcher` devolve `-100` para
 * toda opção, então o `boost` é o **único** critério de ordenação — é esta tabela que
 * decide o que aparece primeiro.
 *
 * **A amplitude é deliberada:** no máximo 150 pontos entre os extremos, abaixo dos 200 da
 * penalidade de case-fold. Assim, quando o usuário digita, um casamento limpo numa
 * categoria desfavorecida ainda ganha de um casamento torto numa favorecida — o contexto
 * ordena, mas nunca atropela a qualidade do match. Alargar além de ±100 quebra isso.
 */
export const CLAUSE_BOOSTS: Record<
  SqlClause,
  Record<SqlCompletionCategory, ClauseBoost>
> = {
  start: { column: -60, table: -30, keyword: 30, function: -20, snippet: 80 },
  select: { column: 70, table: -10, keyword: 0, function: 30, snippet: -60 },
  where: { column: 70, table: -20, keyword: 0, function: 20, snippet: -60 },
  group: { column: 70, table: -20, keyword: 0, function: 10, snippet: -60 },
  having: { column: 70, table: -20, keyword: 0, function: 30, snippet: -60 },
  order: { column: 70, table: -20, keyword: 10, function: 10, snippet: -60 },
  on: { column: 70, table: 30, keyword: 0, function: 10, snippet: -60 },
  set: { column: 70, table: -30, keyword: -10, function: 20, snippet: -60 },
  returning: { column: 70, table: -20, keyword: 0, function: 20, snippet: -60 },
  'insert-columns': {
    column: 90,
    table: -60,
    keyword: -40,
    function: -40,
    snippet: -60,
  },
  // Posição de tabela: coluna ali é ruído puro, então nem é oferecida.
  from: { column: null, table: 80, keyword: -10, function: -40, snippet: -60 },
  join: { column: null, table: 80, keyword: -10, function: -40, snippet: -60 },
  into: { column: null, table: 80, keyword: -10, function: -40, snippet: -60 },
  update: { column: null, table: 80, keyword: -10, function: -40, snippet: -60 },
  values: { column: 20, table: -50, keyword: 10, function: 30, snippet: -60 },
  limit: { column: -60, table: -60, keyword: 10, function: -20, snippet: -60 },
};

export function boostFor(
  clause: SqlClause,
  category: SqlCompletionCategory,
): ClauseBoost {
  return CLAUSE_BOOSTS[clause][category];
}

const SECTION_LABEL: Record<SqlCompletionCategory, string> = {
  column: 'Colunas',
  table: 'Tabelas',
  keyword: 'Palavras-chave',
  function: 'Funções',
  snippet: 'Modelos',
};

/**
 * Agrupar só vale a pena quando o pedido é explícito e nada foi digitado — que é o gesto
 * do atalho. Aí todos os scores de match são iguais e o cabeçalho é ganho puro.
 *
 * Com algo digitado, não: o deslocamento de seção é `-1e5`, aplicado depois do score, e
 * aniquilaria a relevância (digitar `cou` mostraria "Colunas" vazio e `COUNT` soterrado).
 */
export function shouldSection(context: CompletionContext): boolean {
  return context.explicit && context.matchBefore(/\w+/) === null;
}

/**
 * Seção da categoria. O `rank` sai da própria tabela de boost (rank menor renderiza mais
 * acima), então uma tabela só governa a ordem dentro do grupo e a ordem dos grupos.
 */
export function sectionFor(
  clause: SqlClause,
  category: SqlCompletionCategory,
): { name: string; rank: number } {
  return {
    name: SECTION_LABEL[category],
    rank: -(boostFor(clause, category) ?? 0),
  };
}

/** Aplica boost (e seção, quando cabe) a uma lista de opções, sempre em cópias. */
export function rankOptions(
  options: readonly Completion[],
  clause: SqlClause,
  categoryOf: (option: Completion) => SqlCompletionCategory,
  withSection: boolean,
): Completion[] {
  const ranked: Completion[] = [];

  for (const option of options) {
    const category = categoryOf(option);
    const boost = boostFor(clause, category);

    if (boost === null) continue;

    // Cópia obrigatória: `completeFromList` e `keywordCompletionSource` devolvem sempre a
    // mesma referência de array, e mutar corromperia a lista para o resto da sessão.
    ranked.push(
      withSection
        ? { ...option, boost, section: sectionFor(clause, category) }
        : { ...option, boost },
    );
  }

  return ranked;
}

/**
 * Envolve uma source externa (as keywords do dialeto, as nossas estáticas) aplicando o
 * ranqueamento da cláusula sob o cursor.
 *
 * Fora do nível de topo devolve `null`: depois de um qualificador (`u.`, `u.na`) só cabem
 * colunas. Isso também corrige o `ifNotIn` do lang-sql, que só barra quando o nó sob o
 * cursor **é** o ponto — em `u.na` o nó é o `Identifier` dentro do `CompositeIdentifier`.
 */
export function withClauseBoost(
  source: CompletionSource,
  categoryOf: SqlCompletionCategory | ((option: Completion) => SqlCompletionCategory),
): CompletionSource {
  const resolveCategory =
    typeof categoryOf === 'function' ? categoryOf : () => categoryOf;

  // Memo por (identidade do array de opções, cláusula): são 831 keywords, e sem isso o
  // remapeamento rodaria a cada tecla.
  let cachedOptions: readonly Completion[] | null = null;
  let byClause = new Map<SqlClause, Completion[]>();

  const rank = (
    result: CompletionResult,
    context: CompletionContext,
  ): CompletionResult | null => {
    if (result.options.length === 0) return result;

    const { atTopLevel, clause } = getStatementContextAt(
      context.state,
      context.pos,
    );

    if (!atTopLevel) return null;

    const withSection = shouldSection(context);

    if (result.options !== cachedOptions) {
      cachedOptions = result.options;
      byClause = new Map();
    }

    // A variante com seção só aparece no gesto explícito, que é raro — não vale cachear.
    if (withSection) {
      const ranked = rankOptions(result.options, clause, resolveCategory, true);
      return ranked.length === 0 ? null : { ...result, options: ranked };
    }

    let ranked = byClause.get(clause);

    if (!ranked) {
      ranked = rankOptions(result.options, clause, resolveCategory, false);
      byClause.set(clause, ranked);
    }

    return ranked.length === 0 ? null : { ...result, options: ranked };
  };

  return context => {
    const result = source(context);

    if (!result) return null;

    if (result instanceof Promise) {
      return result.then(resolved =>
        resolved ? rank(resolved, context) : null,
      );
    }

    return rank(result, context);
  };
}
