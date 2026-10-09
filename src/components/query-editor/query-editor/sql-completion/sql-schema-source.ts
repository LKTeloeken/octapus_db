import type {
  Completion,
  CompletionContext,
  CompletionSource,
} from '@codemirror/autocomplete';
import { PostgreSQL, schemaCompletionSource } from '@codemirror/lang-sql';
import type { EditorState, Extension } from '@codemirror/state';
import { EditorView } from '@codemirror/view';
import type {
  ColumnInfo,
  DatabaseStructure,
} from '@/api/types/structure.types';
import {
  boostFor,
  rankOptions,
  sectionFor,
  shouldSection,
} from './sql-clause-boost';
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

/** Nomes de schema por chamada; batendo o teto, a lista é refeita a cada tecla. */
const SCHEMA_COMPLETION_LIMIT = 50;

/** Prefixo mínimo para sugerir schemas fora do namespace (milhares de tenants). */
const SCHEMA_PREFIX_MIN = 2;

const PLAIN_IDENTIFIER = /^[a-z_][a-z_\d]*$/;

const PREFETCH_DELAY_MS = 250;

interface StatementTable {
  ref: StatementTableRef;
  target: ResolvedTable;
}

/** Tabelas do statement que existem na estrutura, com os nomes canônicos. */
function resolveStatementTables(
  structure: DatabaseStructure,
  refs: StatementTableRef[],
  defaultSchema?: string | null,
): StatementTable[] {
  const resolved: StatementTable[] = [];

  for (const ref of refs) {
    const target = resolveTable(
      structure,
      ref.table,
      ref.schemaHint ?? defaultSchema ?? undefined,
    );

    if (target) {
      resolved.push({ ref, target });
    }

    if (resolved.length === MAX_TABLES_PER_COMPLETION) break;
  }

  return resolved;
}

/**
 * O que vem antes do ponto no cursor (`tenant_42.or|` → `tenant_42`; num caminho
 * `schema.tabela.|`, o schema). Aspas saem.
 */
function qualifierAt(context: CompletionContext): string | null {
  const match = context.matchBefore(
    /(?:"[^"]+"|[\w$]+)(?:\.(?:"[^"]+"|[\w$]+))?\.[\w$]*$/,
  );
  if (!match) return null;
  const [first] = match.text.split('.');
  return first.startsWith('"') ? first.slice(1, -1) : first;
}

/**
 * O que o statement precisa na estrutura quente: schemas citados (qualificador
 * ou `schema.tabela`) e tabelas sem schema.
 *
 * Um qualificador que é alias não é schema. Se é tabela, quem sabe é `isTable`:
 * em `FROM tenant_42.|` o statement ainda lê `tenant_42` como tabela (nada veio
 * depois do ponto), então a source pergunta à estrutura quente; sem ela, vale o
 * que o statement diz.
 */
export function warmNeeds(
  refs: StatementTableRef[],
  qualifier: string | null,
  isTable?: (name: string) => boolean,
): { schemas: string[]; tables: string[] } {
  const lower = (name: string) => name.toLowerCase();
  const aliases = new Set(
    refs
      .map(ref => ref.alias)
      .filter((alias): alias is string => !!alias)
      .map(lower),
  );
  const statementTables = new Set(refs.map(ref => lower(ref.table)));
  const looksLikeTable =
    isTable ?? ((name: string) => statementTables.has(lower(name)));

  const schemas = new Set(
    refs.map(ref => ref.schemaHint).filter((hint): hint is string => !!hint),
  );
  const qualifierIsSchema =
    !!qualifier && !aliases.has(lower(qualifier)) && !looksLikeTable(qualifier);
  if (qualifierIsSchema) schemas.add(qualifier);

  const tables = new Set(
    refs
      .filter(ref => !ref.schemaHint)
      .map(ref => ref.table)
      .filter(
        table => !(qualifierIsSchema && lower(table) === lower(qualifier)),
      ),
  );
  return { schemas: Array.from(schemas), tables: Array.from(tables) };
}

/** Uma relação com esse nome existe em algum schema da estrutura (quente). */
function hasTable(
  structure: DatabaseStructure | undefined,
  name: string,
): boolean {
  const lower = name.toLowerCase();
  return !!structure?.schemas.some(schema =>
    schema.tables.some(table => table.name.toLowerCase() === lower),
  );
}

/** Colunas de verdade (cache) ou, enquanto não chegam, as provisórias. */
type ColumnLookup = (schema: string, table: string) => ColumnInfo[] | undefined;

/** Colunas emprestadas de outro tenant: o tipo avisa que é prévia. */
function asPreview(columns: ColumnInfo[], from: string): ColumnInfo[] {
  return columns.map(column => ({
    ...column,
    dataType: `${column.dataType} · prévia de ${from}`,
  }));
}

/** Colunas das tabelas do statement, para injetar em posição sem qualificador. */
function statementColumnOptions(
  tables: StatementTable[],
  peek: ColumnLookup,
  taken: ReadonlySet<string>,
  boost: number,
  section: Completion['section'],
): Completion[] {
  const options: Completion[] = [];
  const seen = new Set<string>();

  for (const { ref, target } of tables) {
    const columns = peek(target.schema, target.table);

    if (!columns) continue;

    for (const column of columns) {
      if (seen.has(column.name) || taken.has(column.name)) continue;

      seen.add(column.name);

      const option = columnCompletion(column, boost, ref.alias ?? target.table);

      options.push(section ? { ...option, section } : option);
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
    defaultSchema: string | null;
    previews: string;
    source: CompletionSource;
  } | null = null;

  const langSourceFor = (
    structure: DatabaseStructure,
    previews: ReadonlyMap<string, ColumnInfo[]>,
  ): CompletionSource => {
    const version = ports.getColumnsVersion();
    const defaultSchema = ports.getDefaultSchema();
    const previewKey = Array.from(previews.keys()).join('\u0001');

    if (
      memo &&
      memo.structure === structure &&
      memo.version === version &&
      memo.defaultSchema === defaultSchema &&
      memo.previews === previewKey
    ) {
      return memo.source;
    }

    const columns = new Map<string, ColumnInfo[]>();

    for (const schema of structure.schemas) {
      for (const table of schema.tables) {
        const loaded =
          ports.peekColumns(schema.name, table.name) ??
          previews.get(tableKey(schema.name, table.name));

        if (loaded) {
          columns.set(tableKey(schema.name, table.name), loaded);
        }
      }
    }

    const source = schemaCompletionSource({
      dialect: PostgreSQL,
      schema: buildSqlNamespace(structure, columns),
      defaultSchema:
        defaultSchema ??
        (structure.schemas.some(schema => schema.name === 'public')
          ? 'public'
          : undefined),
    });

    memo = { structure, version, defaultSchema, previews: previewKey, source };

    return source;
  };

  return async context => {
    const { tables, atTopLevel, clause } = getStatementContextAt(
      context.state,
      context.pos,
    );

    // Catálogo no backend: traz para a estrutura quente o que o statement cita
    if (ports.warm) {
      await ports.warm(
        warmNeeds(tables, qualifierAt(context), name =>
          hasTable(ports.getStructure(), name),
        ),
      );
      if (context.aborted) return null;
    }

    const structure = ports.getStructure();

    if (!structure) return null;

    const statementTables = resolveStatementTables(
      structure,
      tables,
      ports.getDefaultSchema(),
    );
    const columnBoost = boostFor(clause, 'column');

    // Em posição de tabela (`FROM |`, `JOIN |`) coluna é ruído: não injeta e nem paga a
    // ida ao banco para carregá-las.
    const missing =
      columnBoost === null
        ? []
        : statementTables.filter(
            entry =>
              !ports.peekColumns(entry.target.schema, entry.target.table),
          );

    // Outro tenant já tem as colunas desta tabela: saem na hora, como prévia, e as
    // de verdade chegam em segundo plano (a próxima tecla já usa as reais)
    const previews = new Map<string, ColumnInfo[]>();
    const toWait = missing.filter(entry => {
      const similar = ports.peekSimilarColumns?.(
        entry.target.schema,
        entry.target.table,
      );
      if (!similar) return true;
      previews.set(
        tableKey(entry.target.schema, entry.target.table),
        asPreview(similar.columns, similar.from),
      );
      void ports.ensureColumns(entry.target.schema, entry.target.table);
      return false;
    });

    if (toWait.length > 0) {
      // Sem resultado parcial: o `validFor` da lib é /^\w*$/, então a source não seria
      // reexecutada no próximo caractere e as colunas nunca apareceriam.
      await Promise.all(
        toWait.map(entry =>
          ports.ensureColumns(entry.target.schema, entry.target.table),
        ),
      );

      if (context.aborted) return null;
    }

    const peek: ColumnLookup = (schema, table) =>
      ports.peekColumns(schema, table) ?? previews.get(tableKey(schema, table));

    const base = await langSourceFor(structure, previews)(context);

    if (!base) return null;

    // Depois de um ponto o lang-sql não diz se as opções são colunas (`u.`) ou tabelas
    // (`public.`) — sem essa informação o boost por cláusula não pode ser aplicado, e
    // aplicá-lo às cegas quebraria `FROM public.|`. O conjunto já está estreito de todo
    // jeito, então devolvemos como veio.
    if (!atTopLevel) return base;

    const withSection = shouldSection(context);
    const tableBoost = boostFor(clause, 'table');

    // O dedupe roda antes dos boosts de propósito: a chave de dedupe do `sortOptions`
    // inclui o `boost`, então inverter a ordem deixaria duplicatas passarem.
    const taken = new Set(
      base.options.map(option =>
        typeof option.label === 'string' ? option.label : '',
      ),
    );
    const extra =
      columnBoost === null
        ? []
        : statementColumnOptions(
            statementTables,
            peek,
            taken,
            columnBoost,
            withSection ? sectionFor(clause, 'column') : undefined,
          );

    const rankedBase =
      tableBoost === null
        ? []
        : rankOptions(base.options, clause, () => 'table', withSection);

    // Schemas fora da estrutura quente (milhares de tenants), por prefixo
    const word = context.matchBefore(/[\w$]+$/);
    let schemaOptions: Completion[] = [];
    let complete = true;
    if (
      ports.completeSchemas &&
      tableBoost !== null &&
      word &&
      word.text.length >= SCHEMA_PREFIX_MIN
    ) {
      const names = await ports.completeSchemas(word.text);
      if (context.aborted) return null;
      complete = names.length < SCHEMA_COMPLETION_LIMIT;
      const section = withSection ? sectionFor(clause, 'table') : undefined;
      schemaOptions = names
        .filter(name => !taken.has(name))
        .map(name => ({
          label: name,
          type: 'namespace',
          detail: 'schema',
          boost: tableBoost,
          apply: PLAIN_IDENTIFIER.test(name) ? undefined : `"${name}"`,
          ...(section ? { section } : {}),
        }));
    }

    return {
      ...base,
      options: rankedBase.concat(schemaOptions, extra),
      // Lista de schemas cortada no teto: a próxima tecla precisa buscar de novo
      ...(complete ? {} : { validFor: undefined }),
    };
  };
}

/**
 * Aquece o cache das tabelas do statement enquanto o usuário digita, para a primeira
 * sugestão não pagar a ida ao banco. É melhor esforço: nada aqui bloqueia o editor.
 * Com catálogo no backend, aquece também a estrutura quente (schemas citados).
 */
export function sqlPrefetchListener(ports: SqlCompletionPorts): Extension {
  let timer: ReturnType<typeof setTimeout> | null = null;

  const prefetch = async (state: EditorState) => {
    const { tables } = getStatementContextAt(state, state.selection.main.head);

    if (ports.warm) await ports.warm(warmNeeds(tables, null));

    const structure = ports.getStructure();

    if (!structure) return;

    for (const { target } of resolveStatementTables(
      structure,
      tables,
      ports.getDefaultSchema(),
    )) {
      if (!ports.peekColumns(target.schema, target.table)) {
        void ports.ensureColumns(target.schema, target.table);
      }
    }
  };

  return EditorView.updateListener.of(update => {
    if (!update.docChanged) return;

    if (timer !== null) clearTimeout(timer);

    const state = update.state;

    timer = setTimeout(() => {
      timer = null;
      void prefetch(state);
    }, PREFETCH_DELAY_MS);
  });
}
