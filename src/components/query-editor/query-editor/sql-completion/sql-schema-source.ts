import type { Completion, CompletionSource } from '@codemirror/autocomplete';
import { PostgreSQL, schemaCompletionSource } from '@codemirror/lang-sql';
import type { Extension } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import type {
  ColumnInfo,
  DatabaseStructure,
} from '@/api/types/structure.types';
import {
  buildSqlNamespace,
  columnCompletion,
  resolveTable,
  tableKey,
} from './sql-namespace';
import { getStatementContextAt } from './sql-statement-context';
import type {
  ResolvedTable,
  SqlCompletionPorts,
  StatementTableRef,
} from './sql-completion.types';

/** Teto de tabelas por invocação, para um JOIN grande não virar rajada de fetch. */
const MAX_TABLES_PER_COMPLETION = 8;

/** As colunas do statement ganham do resto. O score do CodeMirror soma o boost. */
const STATEMENT_COLUMN_BOOST = 50;

const PREFETCH_DELAY_MS = 250;

interface StatementTable {
  ref: StatementTableRef;
  target: ResolvedTable;
}

/** Tabelas do statement que existem na estrutura, com os nomes canônicos. */
function resolveStatementTables(
  structure: DatabaseStructure,
  refs: StatementTableRef[],
): StatementTable[] {
  const resolved: StatementTable[] = [];

  for (const ref of refs) {
    const target = resolveTable(structure, ref.table, ref.schemaHint);

    if (target) {
      resolved.push({ ref, target });
    }

    if (resolved.length === MAX_TABLES_PER_COMPLETION) break;
  }

  return resolved;
}

/** Colunas das tabelas do statement, para injetar em posição sem qualificador. */
function statementColumnOptions(
  tables: StatementTable[],
  ports: SqlCompletionPorts,
  taken: ReadonlySet<string>,
): Completion[] {
  const options: Completion[] = [];
  const seen = new Set<string>();

  for (const { ref, target } of tables) {
    const columns = ports.peekColumns(target.schema, target.table);

    if (!columns) continue;

    for (const column of columns) {
      if (seen.has(column.name) || taken.has(column.name)) continue;

      seen.add(column.name);
      options.push(
        columnCompletion(
          column,
          STATEMENT_COLUMN_BOOST,
          ref.alias ?? target.table,
        ),
      );
    }
  }

  return options;
}

/**
 * Source de schema do editor SQL.
 *
 * Delega a `schemaCompletionSource` do lang-sql — que já resolve alias (`from users u`,
 * `from users AS u`), caminhos pontuados e aspas — e acrescenta o que ela não faz:
 * injetar, em posição sem qualificador, as colunas das tabelas citadas no statement.
 *
 * O namespace é compilado de uma vez pela lib, então a source dela precisa ser
 * reconstruída quando colunas novas chegam ao cache (daí o `getColumnsVersion`).
 */
export function createSqlSchemaSource(
  ports: SqlCompletionPorts,
): CompletionSource {
  let memo: {
    structure: DatabaseStructure;
    version: number;
    source: CompletionSource;
  } | null = null;

  const langSourceFor = (structure: DatabaseStructure): CompletionSource => {
    const version = ports.getColumnsVersion();

    if (memo && memo.structure === structure && memo.version === version) {
      return memo.source;
    }

    const columns = new Map<string, ColumnInfo[]>();

    for (const schema of structure.schemas) {
      for (const table of schema.tables) {
        const loaded = ports.peekColumns(schema.name, table.name);

        if (loaded) {
          columns.set(tableKey(schema.name, table.name), loaded);
        }
      }
    }

    const source = schemaCompletionSource({
      dialect: PostgreSQL,
      schema: buildSqlNamespace(structure, columns),
      defaultSchema: structure.schemas.some(schema => schema.name === 'public')
        ? 'public'
        : undefined,
    });

    memo = { structure, version, source };

    return source;
  };

  return async context => {
    const structure = ports.getStructure();

    if (!structure) return null;

    const { tables, atTopLevel } = getStatementContextAt(
      context.state,
      context.pos,
    );
    const statementTables = resolveStatementTables(structure, tables);

    const missing = statementTables.filter(
      entry => !ports.peekColumns(entry.target.schema, entry.target.table),
    );

    if (missing.length > 0) {
      // Sem resultado parcial: o `validFor` da lib é /^\w*$/, então a source não seria
      // reexecutada no próximo caractere e as colunas nunca apareceriam.
      await Promise.all(
        missing.map(entry =>
          ports.ensureColumns(entry.target.schema, entry.target.table),
        ),
      );

      if (context.aborted) return null;
    }

    const base = await langSourceFor(structure)(context);

    if (!base) return null;
    if (!atTopLevel) return base;

    const taken = new Set(
      base.options.map(option =>
        typeof option.label === 'string' ? option.label : '',
      ),
    );
    const extra = statementColumnOptions(statementTables, ports, taken);

    if (extra.length === 0) return base;

    return { ...base, options: base.options.concat(extra) };
  };
}

/**
 * Aquece o cache das tabelas do statement enquanto o usuário digita, para a primeira
 * sugestão não pagar a ida ao banco. É melhor esforço: nada aqui bloqueia o editor.
 */
export function sqlPrefetchListener(ports: SqlCompletionPorts): Extension {
  let timer: ReturnType<typeof setTimeout> | null = null;

  return EditorView.updateListener.of(update => {
    if (!update.docChanged) return;

    if (timer !== null) clearTimeout(timer);

    const state = update.state;

    timer = setTimeout(() => {
      timer = null;

      const structure = ports.getStructure();

      if (!structure) return;

      const { tables } = getStatementContextAt(
        state,
        state.selection.main.head,
      );

      for (const { target } of resolveStatementTables(structure, tables)) {
        if (!ports.peekColumns(target.schema, target.table)) {
          void ports.ensureColumns(target.schema, target.table);
        }
      }
    }, PREFETCH_DELAY_MS);
  });
}
