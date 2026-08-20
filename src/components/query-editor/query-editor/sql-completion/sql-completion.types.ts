import type {
  ColumnInfo,
  DatabaseStructure,
} from '@/api/types/structure.types';

/** Tabela citada no statement sob o cursor (FROM, JOIN, UPDATE, INSERT INTO, DELETE FROM). */
export interface StatementTableRef {
  /** Qualificador de schema escrito pelo usuário (`public.users`), quando houver. */
  schemaHint?: string;
  table: string;
  alias?: string;
}

/**
 * Cláusula onde o cursor está. Governa qual categoria de sugestão vale mais.
 * `start` absorve o caso indefinido: documento vazio, logo após `;`, ou statement sem
 * nenhuma cláusula reconhecida antes do cursor.
 */
export type SqlClause =
  | 'start'
  | 'select'
  | 'from'
  | 'join'
  | 'on'
  | 'where'
  | 'group'
  | 'having'
  | 'order'
  | 'limit'
  | 'set'
  | 'update'
  | 'into'
  | 'insert-columns'
  | 'values'
  | 'returning';

export interface SqlStatementContext {
  tables: StatementTableRef[];
  /**
   * Cursor sem qualificador pontuado antes dele — é onde faz sentido injetar as colunas
   * das tabelas do statement (`select |`, `where |`, `set |`). Depois de um ponto (`u.`)
   * quem resolve é o `schemaCompletionSource` do lang-sql.
   *
   * **Invariante:** o boost por cláusula só se aplica quando isto é `true`. Depois de um
   * ponto não dá para saber se as opções são colunas (`u.`) ou tabelas (`public.`).
   */
  atTopLevel: boolean;
  clause: SqlClause;
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
