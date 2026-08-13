import type { ColumnInfo, DatabaseStructure } from '@/api/types/structure.types';

/** Tabela citada no statement sob o cursor (FROM, JOIN, UPDATE, INSERT INTO, DELETE FROM). */
export interface StatementTableRef {
  /** Qualificador de schema escrito pelo usuário (`public.users`), quando houver. */
  schemaHint?: string;
  table: string;
  alias?: string;
}

export interface SqlStatementContext {
  tables: StatementTableRef[];
  /**
   * Cursor sem qualificador pontuado antes dele — é onde faz sentido injetar as colunas
   * das tabelas do statement (`select |`, `where |`, `set |`). Depois de um ponto (`u.`)
   * quem resolve é o `schemaCompletionSource` do lang-sql.
   */
  atTopLevel: boolean;
}

/** Schema + tabela como estão gravados na estrutura do banco (nomes canônicos). */
export interface ResolvedTable {
  schema: string;
  table: string;
}

/**
 * Porta de dados do autocomplete. É implementada na camada de feature (React Query) e
 * consumida aqui, para o componente não conhecer `invoke` nem o cache de queries.
 */
export interface SqlCompletionPorts {
  getStructure: () => DatabaseStructure | undefined;
  /** Leitura síncrona do cache; `undefined` = colunas ainda não carregadas. */
  peekColumns: (schema: string, table: string) => ColumnInfo[] | undefined;
  /** Garante as colunas no cache (busca só se faltar). Nunca rejeita. */
  ensureColumns: (
    schema: string,
    table: string,
  ) => Promise<ColumnInfo[] | undefined>;
  /** Sobe a cada conjunto de colunas que entra no cache — chave de memoização. */
  getColumnsVersion: () => number;
}
