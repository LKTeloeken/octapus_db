import type { Completion } from '@codemirror/autocomplete';
import type { SQLNamespace } from '@codemirror/lang-sql';
import type {
  ColumnInfo,
  DatabaseStructure,
} from '@/api/types/structure.types';
import type { ResolvedTable } from './sql-completion.types';

/** Identificador que o Postgres aceita sem aspas. */
const PLAIN_IDENTIFIER = /^[a-z_][a-z_\d]*$/;

/** Chave do mapa de colunas carregadas. `\0` não aparece em nome de objeto. */
export function tableKey(schema: string, table: string): string {
  return `${schema}\u0000${table}`;
}

/**
 * O `addNamespaceObject` do lang-sql fatia a chave em `.` para montar os níveis, então
 * um objeto chamado `my.table` viraria dois níveis se não escapássemos o ponto.
 */
function escapeKey(name: string): string {
  return name.replace(/\./g, '\\.');
}

export function columnCompletion(
  column: ColumnInfo,
  boost?: number,
  origin?: string,
): Completion {
  const flag = column.isPrimaryKey ? ' · pk' : column.isForeignKey ? ' · fk' : '';
  const detail = origin
    ? `${column.dataType}${flag} · ${origin}`
    : `${column.dataType}${flag}`;

  return {
    label: column.name,
    type: 'property',
    detail,
    boost,
    // A lib só cita identificadores sozinha quando a opção é string; a nossa é objeto.
    apply: PLAIN_IDENTIFIER.test(column.name) ? undefined : `"${column.name}"`,
  };
}

/**
 * Monta o `SQLNamespace` que o `schemaCompletionSource` compila. Tabela sem colunas
 * carregadas entra como lista vazia — ela continua sendo listada como tabela, porque o
 * `CompletionLevel.child()` registra o nome no nível de cima.
 */
export function buildSqlNamespace(
  structure: DatabaseStructure,
  columns: ReadonlyMap<string, ColumnInfo[]>,
): SQLNamespace {
  const namespace: { [name: string]: SQLNamespace } = {};

  for (const schema of structure.schemas) {
    const tables: { [name: string]: SQLNamespace } = {};

    for (const table of schema.tables) {
      const loaded = columns.get(tableKey(schema.name, table.name));

      tables[escapeKey(table.name)] = loaded
        ? loaded.map(column => columnCompletion(column))
        : [];
    }

    namespace[escapeKey(schema.name)] = tables;
  }

  return namespace;
}

/**
 * Resolve o nome escrito na query para os nomes canônicos da estrutura. Sem `search_path`
 * no app, a desambiguação é `public` primeiro — é o default do Postgres. Esta função é a
 * única costura caso um dia a aba ganhe um `search_path` próprio.
 */
export function resolveTable(
  structure: DatabaseStructure,
  table: string,
  schemaHint?: string,
): ResolvedTable | null {
  const lowerTable = table.toLowerCase();

  const matchTable = (candidate: string) =>
    candidate === table || candidate.toLowerCase() === lowerTable;

  if (schemaHint) {
    const lowerHint = schemaHint.toLowerCase();
    const schema = structure.schemas.find(
      entry =>
        entry.name === schemaHint || entry.name.toLowerCase() === lowerHint,
    );

    if (!schema) return null;

    const found = schema.tables.find(entry => matchTable(entry.name));

    return found ? { schema: schema.name, table: found.name } : null;
  }

  const candidates: ResolvedTable[] = [];

  for (const schema of structure.schemas) {
    const found = schema.tables.find(entry => matchTable(entry.name));

    if (found) {
      candidates.push({ schema: schema.name, table: found.name });
    }
  }

  if (candidates.length === 0) return null;

  return candidates.find(entry => entry.schema === 'public') ?? candidates[0];
}
