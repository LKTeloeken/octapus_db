import { Facet, type Extension } from '@codemirror/state';
import type { SqlClause, StatementTableRef } from './sql-completion.types';

/**
 * Contexto implícito de um editor que guarda só um **pedaço** de statement — o filtro
 * WHERE da aba de tabela, por exemplo. O texto não tem `FROM`, então a varredura do
 * statement não acharia tabela nenhuma e cairia em `start`; aqui o editor declara de
 * fora qual tabela está em jogo e em que cláusula o texto mora.
 */
export interface SqlFragmentContext {
  tables: StatementTableRef[];
  clause: SqlClause;
}

/** Último valor vence; sem valor, o editor é um statement completo como sempre. */
export const sqlFragmentContextFacet = Facet.define<
  SqlFragmentContext,
  SqlFragmentContext | null
>({
  combine: values => (values.length > 0 ? values[values.length - 1] : null),
});

export function sqlFragmentContext(context: SqlFragmentContext): Extension {
  return sqlFragmentContextFacet.of(context);
}
