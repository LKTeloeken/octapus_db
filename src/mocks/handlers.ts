import { Channel } from '@tauri-apps/api/core';
import { RustCommand } from '@/api/commands';
import type { TableDataRequest } from '@/api/types/browse.types';
import type {
  EditableInfo,
  QueryMessage,
  QueryOptions,
  QueryResult,
  RowEdit,
  RowInsert,
  StatementResult,
} from '@/api/types/query.types';
import type { Server, ServerInput } from '@/api/types/server.types';
import type {
  ColumnInfo,
  DatabaseInfo,
  DatabaseStructure,
  IndexInfo,
  SchemaInfo,
  TableInfo,
} from '@/api/types/structure.types';
import {
  CAPABILITIES,
  buildDatabasesFor,
  getDataset,
  qualify,
  requireDatabase,
  requireServerEntry,
  requireTable,
  tableRows,
  toColumnInfo,
  type MockServerEntry,
  type MockTable,
} from './data';
import {
  applyDeletes,
  applyEdits,
  applyInserts,
  emptyResult,
  selectFrom,
} from './engine';
import { useMockStore } from './mock-store';

type Args = Record<string, unknown>;
export type MockHandler = (args: Args) => unknown | Promise<unknown>;

const isEmptyMode = () => useMockStore.getState().emptyMode;

// ── Servidores ──────────────────────────────────────────────────────────────

/** O Rust marca `password` como `skip_serializing`: ela nunca volta pro front */
const publicServer = (server: Server): Server => ({ ...server });

const serverFromInput = (
  id: number,
  input: ServerInput,
  createdAt: number,
): Server => ({
  id,
  name: input.name,
  dbType: input.dbType,
  host: input.host,
  port: input.port,
  username: input.username,
  defaultDatabase: input.defaultDatabase ?? null,
  sslEnabled: input.sslEnabled ?? false,
  connectionUri: input.connectionUri ?? null,
  createdAt,
});

const serverHandlers: Record<string, MockHandler> = {
  [RustCommand.GetAllServers]: (): Server[] =>
    isEmptyMode()
      ? []
      : getDataset().servers.map(entry => publicServer(entry.server)),

  [RustCommand.GetServer]: ({ id }: Args): Server =>
    publicServer(requireServerEntry(id as number).server),

  [RustCommand.CreateServer]: ({ input }: Args): Server => {
    const dataset = getDataset();
    const typed = input as ServerInput;
    const server = serverFromInput(
      dataset.nextServerId++,
      typed,
      Math.floor(Date.now() / 1000),
    );

    dataset.servers.push({
      server,
      capabilities: CAPABILITIES[typed.dbType],
      // Servidor novo também precisa ser navegável, senão a árvore fica morta.
      databases: buildDatabasesFor(typed.dbType),
    });

    return publicServer(server);
  },

  [RustCommand.UpdateServer]: ({ id, input }: Args): Server => {
    const entry = requireServerEntry(id as number);
    const typed = input as ServerInput;
    const dbTypeChanged = entry.server.dbType !== typed.dbType;

    entry.server = serverFromInput(
      entry.server.id,
      typed,
      entry.server.createdAt,
    );
    entry.capabilities = CAPABILITIES[typed.dbType];
    if (dbTypeChanged) entry.databases = buildDatabasesFor(typed.dbType);

    return publicServer(entry.server);
  },

  [RustCommand.DeleteServer]: ({ id }: Args): void => {
    const dataset = getDataset();
    const index = dataset.servers.findIndex(entry => entry.server.id === id);
    if (index < 0) throw `Not found: Server with id ${String(id)} not found`;
    dataset.servers.splice(index, 1);
  },
};

// ── Conexão ─────────────────────────────────────────────────────────────────

const connectionHandlers: Record<string, MockHandler> = {
  [RustCommand.Connect]: ({ serverId }: Args): boolean => {
    requireServerEntry(serverId as number);
    return true;
  },

  [RustCommand.TestConnection]: ({ serverId }: Args): boolean => {
    requireServerEntry(serverId as number);
    return true;
  },

  [RustCommand.Disconnect]: (): void => undefined,

  /** Só o Postgres devolve stats; nos demais o trait default é `None` */
  [RustCommand.GetPoolStats]: ({ serverId }: Args) => {
    const entry = requireServerEntry(serverId as number);
    if (entry.server.dbType !== 'postgres') return null;
    return { size: 5, available: 4, inUse: 1, waiting: 0 };
  },

  [RustCommand.GetCapabilities]: ({ serverId }: Args) =>
    requireServerEntry(serverId as number).capabilities,
};

// ── Estrutura ───────────────────────────────────────────────────────────────

/** Nome do schema usado internamente: '' quando o adapter não tem schemas */
const schemaKey = (entry: MockServerEntry, schema: unknown) =>
  entry.capabilities.hasSchemas ? (schema as string) || 'public' : '';

const toTableInfo = (table: MockTable): TableInfo => ({
  name: table.name,
  schema: table.schema,
  tableType: table.tableType,
  rowEstimate: table.rowEstimate,
});

const structureHandlers: Record<string, MockHandler> = {
  [RustCommand.ListDatabases]: ({ serverId }: Args): DatabaseInfo[] => {
    const entry = requireServerEntry(serverId as number);
    if (isEmptyMode()) return [];
    return entry.databases.map(db => ({
      name: db.name,
      sizeBytes: db.sizeBytes,
    }));
  },

  [RustCommand.ListSchemas]: ({ serverId, database }: Args): SchemaInfo[] => {
    const entry = requireServerEntry(serverId as number);
    // Mongo/Redis caem no default da trait, que devolve lista vazia.
    if (!entry.capabilities.hasSchemas || isEmptyMode()) return [];

    const tables = requireDatabase(entry, database as string).tables;
    const names = Array.from(new Set(tables.map(table => table.schema)));
    return names.map(name => ({
      name,
      tableCount: tables.filter(table => table.schema === name).length,
    }));
  },

  [RustCommand.ListTables]: ({
    serverId,
    database,
    schema,
  }: Args): TableInfo[] => {
    const entry = requireServerEntry(serverId as number);
    if (isEmptyMode()) return [];
    const wanted = schemaKey(entry, schema);
    return requireDatabase(entry, database as string)
      .tables.filter(table => table.schema === wanted)
      .map(toTableInfo);
  },

  [RustCommand.ListColumns]: ({
    serverId,
    database,
    schema,
    table,
  }: Args): ColumnInfo[] => {
    const entry = requireServerEntry(serverId as number);
    const found = requireTable(
      entry,
      database as string,
      schema as string,
      table as string,
    );
    return found.columns.map(toColumnInfo);
  },

  [RustCommand.ListIndexes]: ({
    serverId,
    database,
    schema,
    table,
  }: Args): IndexInfo[] => {
    const entry = requireServerEntry(serverId as number);
    if (!entry.capabilities.supportsIndexes) {
      throw 'Unsupported type: Indexes are not supported for this database';
    }
    return requireTable(
      entry,
      database as string,
      schema as string,
      table as string,
    ).indexes;
  },

  [RustCommand.ListSchemasWithTables]: ({
    serverId,
    database,
  }: Args): DatabaseStructure => {
    const entry = requireServerEntry(serverId as number);
    const fetchedAt = Date.now();
    if (isEmptyMode()) return { schemas: [], fetchedAt };

    const tables = requireDatabase(entry, database as string).tables;
    const names = Array.from(new Set(tables.map(table => table.schema)));

    return {
      // Sem schemas o backend devolve um único grupo de nome vazio, e a árvore
      // achata com `schemas.flatMap(s => s.tables)`.
      schemas: names.map(name => ({
        name,
        tables: tables
          .filter(table => table.schema === name)
          .map(table => ({ name: table.name, tableType: table.tableType })),
      })),
      fetchedAt,
    };
  },
};

// ── Browse ──────────────────────────────────────────────────────────────────

const browseHandlers: Record<string, MockHandler> = {
  [RustCommand.FetchTableData]: ({
    serverId,
    database,
    request,
  }: Args): QueryResult => {
    const entry = requireServerEntry(serverId as number);
    const typed = request as TableDataRequest;
    const table = requireTable(
      entry,
      database as string,
      typed.schema,
      typed.table,
    );

    if (typed.whereExpr && !entry.capabilities.supportsSql) {
      throw 'Unsupported type: Raw filters are only supported on SQL databases';
    }
    if (isEmptyMode()) return emptyResult(table);

    return selectFrom(table, {
      whereExpr: typed.whereExpr,
      sort: typed.sort,
      limit: typed.limit,
      offset: typed.offset,
      countTotal: typed.countTotal,
      unlimited: typed.unlimited,
    });
  },
};

// ── Exportação ──────────────────────────────────────────────────────────────

const exportHandlers: Record<string, MockHandler> = {
  // Sem disco no navegador: o arquivo cai no download em vez de um caminho.
  [RustCommand.WriteExportFile]: ({ path, contents }: Args) => {
    const url = URL.createObjectURL(new Blob([contents as string]));
    const link = document.createElement('a');
    link.href = url;
    link.download = String(path).split('/').pop() ?? 'export';
    link.click();
    URL.revokeObjectURL(url);
  },
};

// ── Sessão do workspace ─────────────────────────────────────────────────────

/** O SQLite do app vira `localStorage` — a origin do mock já é isolada do app real. */
const SESSION_STORAGE_KEY = 'octapus-mock-session';

const sessionHandlers: Record<string, MockHandler> = {
  [RustCommand.LoadSession]: (): string | null =>
    localStorage.getItem(SESSION_STORAGE_KEY),

  [RustCommand.SaveSession]: ({ snapshot }: Args): void => {
    localStorage.setItem(SESSION_STORAGE_KEY, snapshot as string);
  },
};

// ── Editor livre ────────────────────────────────────────────────────────────

/**
 * Não interpretamos SQL de verdade — basta localizar a tabela para devolver um
 * result set coerente. Postgres via `FROM [schema.]tabela`, Mongo via
 * `db.colecao.…` e Redis via o prefixo da chave no comando.
 */
function resolveQueryTable(
  entry: MockServerEntry,
  database: string,
  query: string,
  defaultSchema?: string | null,
): MockTable | null {
  const db = requireDatabase(entry, database);

  if (entry.server.dbType === 'postgres') {
    const match = /\bfrom\s+(?:"?(\w+)"?\.)?"?(\w+)"?/i.exec(query);
    if (!match) return null;
    const [, schema, table] = match;
    const found = db.tables.find(
      candidate =>
        candidate.name === table &&
        (schema
          ? candidate.schema === schema
          : defaultSchema
            ? candidate.schema === defaultSchema
            : true),
    );
    if (!found) {
      throw `Query error: relation "${qualify(schema ?? '', table)}" does not exist`;
    }
    return found;
  }

  if (entry.server.dbType === 'mongodb') {
    const match = /\bdb\.(\w+)\b/i.exec(query);
    if (!match) return null;
    const found = db.tables.find(candidate => candidate.name === match[1]);
    if (!found) throw `Query error: collection "${match[1]}" not found`;
    return found;
  }

  // Redis: o "grupo de chaves" é o prefixo até o primeiro ':'.
  const match = /([\w-]+:)/.exec(query);
  if (!match) return db.tables[0] ?? null;
  return (
    db.tables.find(candidate => candidate.name === match[1]) ??
    db.tables[0] ??
    null
  );
}

const READ_COMMAND =
  /^\s*(select|with|show|explain|table|db\.|get|scan|hgetall|keys|lrange|smembers|zrange)/i;

const queryHandlers: Record<string, MockHandler> = {
  [RustCommand.ExecuteQuery]: async ({
    serverId,
    database,
    query,
    options,
    messages,
  }: Args): Promise<QueryResult> => {
    const entry = requireServerEntry(serverId as number);
    const sql = (query as string) ?? '';
    const opts = (options as QueryOptions | undefined) ?? {};
    const channel = messages as Channel<QueryMessage> | undefined;
    const startedAt = performance.now();

    emitStart(entry, channel);

    if (!READ_COMMAND.test(sql)) {
      // DDL/DML no editor: sem result set, só a linha de conclusão.
      emitFinish(
        entry,
        channel,
        sql.trim().split(/\s+/)[0]?.toUpperCase() ?? 'OK',
        0,
      );
      return {
        ...emptyResult(),
        executionTimeMs: Math.round(performance.now() - startedAt),
      };
    }

    const table = resolveQueryTable(
      entry,
      database as string,
      sql,
      opts.schema,
    );
    if (!table)
      throw `Query error: syntax error at or near "${sql.trim().slice(0, 12)}"`;

    if (isEmptyMode()) {
      emitFinish(entry, channel, 'SELECT', 0);
      return emptyResult(table);
    }

    const limitMatch = /\blimit\s+(\d+)/i.exec(sql);
    const result = selectFrom(
      table,
      {
        limit: limitMatch ? Number(limitMatch[1]) : opts.limit,
        offset: opts.offset,
        countTotal: opts.countTotal,
        unlimited: opts.unlimited,
      },
      startedAt,
    );

    await emitNotices(entry, channel, sql);
    emitFinish(entry, channel, 'SELECT', result.rowCount);

    return result;
  },

  [RustCommand.ExecuteStatement]: ({ serverId }: Args): StatementResult => {
    requireServerEntry(serverId as number);
    return { affectedRows: 1, executionTimeMs: 3 };
  },

  [RustCommand.ExecuteTransaction]: ({
    serverId,
    statements,
  }: Args): StatementResult[] => {
    const entry = requireServerEntry(serverId as number);
    if (!entry.capabilities.supportsTransactions) {
      throw 'Unsupported type: Transactions are not supported for this database';
    }
    return (statements as string[]).map(() => ({
      affectedRows: 1,
      executionTimeMs: 2,
    }));
  },

  [RustCommand.ApplyRowEdits]: ({
    serverId,
    database,
    editable,
    edits,
  }: Args): StatementResult => {
    const { table, startedAt } = editTarget(serverId, database, editable);
    const affectedRows = applyEdits(
      table,
      editable as EditableInfo,
      edits as RowEdit[],
    );
    return {
      affectedRows,
      executionTimeMs: Math.round(performance.now() - startedAt),
    };
  },

  [RustCommand.InsertRows]: ({
    serverId,
    database,
    editable,
    rows,
  }: Args): StatementResult => {
    const { table, startedAt } = editTarget(serverId, database, editable);
    const affectedRows = applyInserts(table, rows as RowInsert[]);
    return {
      affectedRows,
      executionTimeMs: Math.round(performance.now() - startedAt),
    };
  },

  [RustCommand.DeleteRows]: ({
    serverId,
    database,
    editable,
    pkValues,
  }: Args): StatementResult => {
    const { table, startedAt } = editTarget(serverId, database, editable);
    const affectedRows = applyDeletes(
      table,
      editable as EditableInfo,
      pkValues as (string | null)[][],
    );
    return {
      affectedRows,
      executionTimeMs: Math.round(performance.now() - startedAt),
    };
  },

  [RustCommand.CancelQuery]: (): void => undefined,
};

function editTarget(serverId: unknown, database: unknown, editable: unknown) {
  const entry = requireServerEntry(serverId as number);
  if (!entry.capabilities.hasPrimaryKeys) {
    throw 'Unsupported type: Row editing is not supported for this database';
  }
  const typed = editable as EditableInfo;
  const table = requireTable(
    entry,
    database as string,
    typed.schema,
    typed.table,
  );
  tableRows(table);
  return { table, startedAt: performance.now() };
}

// ── Canal de mensagens (RAISE do Postgres) ──────────────────────────────────

/** Teto do backend (`MAX_STREAMED_NOTICES` em commands/queries.rs) */
const MAX_STREAMED_NOTICES = 2_000;

type MessageExtras = Partial<
  Pick<QueryMessage, 'detail' | 'hint' | 'context' | 'sqlState'>
>;

const buildMessage = (
  kind: QueryMessage['kind'],
  severity: string,
  text: string,
  extras: MessageExtras = {},
): QueryMessage => ({
  kind,
  severity,
  message: text,
  detail: extras.detail ?? null,
  hint: extras.hint ?? null,
  context: extras.context ?? null,
  sqlState: extras.sqlState ?? null,
  position: null,
  timestampMs: Date.now(),
});

/** Só o adapter do Postgres alimenta o canal; os outros ignoram o sink */
const streams = (entry: MockServerEntry, channel?: Channel<QueryMessage>) =>
  channel != null && entry.server.dbType === 'postgres';

const send = (channel: Channel<QueryMessage>, payload: QueryMessage) => {
  channel.onmessage(payload);
};

const tick = () => new Promise(resolve => setTimeout(resolve, 0));

function emitStart(entry: MockServerEntry, channel?: Channel<QueryMessage>) {
  if (!streams(entry, channel)) return;
  send(channel!, buildMessage('status', 'STATUS', 'Executando…'));
}

/**
 * Query com `raise` dispara o volume máximo: é o único jeito de exercitar o
 * buffer de requestAnimationFrame do use-query-runner e o corte de 2000.
 */
async function emitNotices(
  entry: MockServerEntry,
  channel: Channel<QueryMessage> | undefined,
  sql: string,
) {
  if (!streams(entry, channel)) return;

  if (!/\braise\b/i.test(sql)) {
    send(channel!, buildMessage('notice', 'NOTICE', 'plano usou índice btree'));
    send(
      channel!,
      buildMessage('warning', 'WARNING', 'consulta sem LIMIT explícito', {
        hint: 'O mock aplicou o limite padrão de 500 linhas.',
      }),
    );
    return;
  }

  for (let i = 1; i <= MAX_STREAMED_NOTICES; i++) {
    send(
      channel!,
      buildMessage('notice', 'NOTICE', `iteração ${i} processada`, {
        context: 'PL/pgSQL function loop_demo() line 7 at RAISE',
      }),
    );
    if (i % 250 === 0) await tick();
  }

  send(
    channel!,
    buildMessage(
      'status',
      'STATUS',
      `Log truncado em ${MAX_STREAMED_NOTICES} mensagens; as demais desta execução foram descartadas`,
    ),
  );
}

function emitFinish(
  entry: MockServerEntry,
  channel: Channel<QueryMessage> | undefined,
  verb: string,
  rowCount: number,
) {
  if (!streams(entry, channel)) return;
  send(channel!, buildMessage('status', 'STATUS', `${verb} ${rowCount}`));
}

// ── Plugins ─────────────────────────────────────────────────────────────────

const pluginHandlers: Record<string, MockHandler> = {
  'plugin:updater|check': () => {
    if (!useMockStore.getState().updateAvailable) return null;
    return {
      rid: 1,
      available: true,
      currentVersion: '0.1.0-beta.16',
      version: '0.1.0-beta.19',
      date: '2026-09-14 12:00:00.000 +00:00:00',
      body: 'Notas de versão simuladas pelo modo mock.',
      rawJson: {},
    };
  },

  'plugin:updater|download_and_install': async ({ onEvent }: Args) => {
    const channel = onEvent as Channel<unknown> | undefined;
    if (!channel) return;

    const total = 24_000_000;
    channel.onmessage({ event: 'Started', data: { contentLength: total } });
    for (let sent = 0; sent < total; sent += total / 20) {
      await new Promise(resolve => setTimeout(resolve, 120));
      channel.onmessage({
        event: 'Progress',
        data: { chunkLength: total / 20 },
      });
    }
    channel.onmessage({ event: 'Finished', data: {} });
  },

  'plugin:process|restart': () => {
    window.location.reload();
  },

  // O diálogo nativo não existe aqui: devolve o caminho que ele devolveria.
  'plugin:dialog|save': ({ options }: Args) => {
    const { defaultPath } = (options ?? {}) as { defaultPath?: string };
    return `/tmp/${defaultPath ?? 'export'}`;
  },

  // Fora do Tauri quem escreve é o navegador, que pode recusar sem gesto do
  // usuário — no mock isso não é erro, o que importa é o fluxo ter chegado aqui.
  'plugin:clipboard-manager|write_text': async ({ text }: Args) => {
    try {
      await navigator.clipboard.writeText(text as string);
    } catch {
      console.info('[octapus-mock] clipboard recusado pelo navegador');
    }
  },
};

export const handlers: Record<string, MockHandler> = {
  ...serverHandlers,
  ...connectionHandlers,
  ...structureHandlers,
  ...browseHandlers,
  ...exportHandlers,
  ...sessionHandlers,
  ...queryHandlers,
  ...pluginHandlers,
};
