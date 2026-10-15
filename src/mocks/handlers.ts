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
import type {
  Server,
  ServerGroup,
  ServerInput,
} from '@/api/types/server.types';
import {
  BACKUP_EVENT,
  type BackupEvent,
  type BackupRequest,
  type PgToolsDetection,
  type PgToolsInstall,
  type PgToolsStatus,
  type RestoreInspection,
  type RestoreRequest,
} from '@/api/types/backup.types';
import type {
  ImportCandidate,
  ImportCommitRequest,
  ImportPreview,
  ImportResult,
  ImportSource,
} from '@/api/types/import.types';
import type { AppSettings } from '@/api/types/settings.types';
import type {
  ColumnInfo,
  DatabaseInfo,
  DatabaseStructure,
  IndexInfo,
  SchemaInfo,
  StructureChange,
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
import { mockCatalog } from './catalog';
import { applyStructure, structureSql } from './structure';
import { parseScope } from './scope';
import { mockEmit, registerListener, unregisterListener } from './events';
import { useMockStore } from './mock-store';
import { MOCK_IMPORT_SOURCES, mockImportFixtures } from './data/imports';

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
  scopeDatabases: input.scopeDatabases?.trim() ? input.scopeDatabases : null,
  scopeSchemas: input.scopeSchemas?.trim() ? input.scopeSchemas : null,
  groupId: input.groupId ?? null,
});

/** Nome aparado, não vazio e único (sem diferenciar maiúsculas), como no Rust */
const validGroupName = (name: string, except?: number): string => {
  const trimmed = name.trim();
  if (!trimmed) throw 'Invalid state: O nome do grupo não pode ficar vazio';
  const taken = getDataset().groups.some(
    group =>
      group.id !== except && group.name.toLowerCase() === trimmed.toLowerCase(),
  );
  if (taken) throw `Already exists: Já existe um grupo chamado "${trimmed}"`;
  return trimmed;
};

const requireGroup = (id: number): ServerGroup => {
  const group = getDataset().groups.find(entry => entry.id === id);
  if (!group) throw `Not found: Group with id ${String(id)} not found`;
  return group;
};

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
    mockCatalog.forgetServer(entry.server.id);

    return publicServer(entry.server);
  },

  [RustCommand.DeleteServer]: ({ id }: Args): void => {
    const dataset = getDataset();
    const index = dataset.servers.findIndex(entry => entry.server.id === id);
    if (index < 0) throw `Not found: Server with id ${String(id)} not found`;
    dataset.servers.splice(index, 1);
  },

  [RustCommand.SetServerGroup]: ({ id, groupId }: Args): void => {
    const entry = requireServerEntry(id as number);
    if (groupId != null) requireGroup(groupId as number);
    entry.server = { ...entry.server, groupId: (groupId as number | null) ?? null };
  },

  [RustCommand.GetAllServerGroups]: (): ServerGroup[] =>
    isEmptyMode()
      ? []
      : [...getDataset().groups].sort((a, b) =>
          a.name.localeCompare(b.name, undefined, { sensitivity: 'base' }),
        ),

  [RustCommand.CreateServerGroup]: ({ name }: Args): ServerGroup => {
    const dataset = getDataset();
    const group: ServerGroup = {
      id: dataset.nextGroupId++,
      name: validGroupName(name as string),
      createdAt: Math.floor(Date.now() / 1000),
    };
    dataset.groups.push(group);
    return group;
  },

  [RustCommand.RenameServerGroup]: ({ id, name }: Args): ServerGroup => {
    const group = requireGroup(id as number);
    group.name = validGroupName(name as string, group.id);
    return { ...group };
  },

  [RustCommand.DeleteServerGroup]: ({ id }: Args): void => {
    const dataset = getDataset();
    requireGroup(id as number);
    dataset.groups = dataset.groups.filter(group => group.id !== id);
    for (const entry of dataset.servers) {
      if (entry.server.groupId === id) entry.server = { ...entry.server, groupId: null };
    }
  },
};

// ── Importar conexões ───────────────────────────────────────────────────────

/** A prévia aberta (no Rust ela guarda as senhas; aqui só os candidatos) */
let mockImportPreview: ImportPreview | null = null;

const importHandlers: Record<string, MockHandler> = {
  [RustCommand.ImportDetectSources]: () => MOCK_IMPORT_SOURCES,

  [RustCommand.ImportPreview]: ({ source, path, text }: Args): ImportPreview => {
    const servers = getDataset().servers.map(entry => entry.server);
    const candidates: ImportCandidate[] = mockImportFixtures(
      source as ImportSource,
      (text as string | null) ?? null,
    ).map((fixture, key) => ({
      ...fixture,
      key,
      duplicateOf:
        servers.find(
          server =>
            server.dbType === fixture.dbType &&
            server.host === fixture.host &&
            server.port === fixture.port &&
            server.username === fixture.username &&
            server.defaultDatabase === fixture.defaultDatabase,
        )?.id ?? null,
    }));
    const groups = Array.from(new Set(candidates.flatMap(c => (c.group ? [c.group] : []))));
    const info = MOCK_IMPORT_SOURCES.find(entry => entry.source === source);
    mockImportPreview = {
      // Mesma faixa do Rust (u32): cabe exato num Number
      token: Math.floor(Math.random() * 2 ** 32),
      source: source as ImportSource,
      path: (path as string | null) ?? info?.path ?? null,
      candidates,
      groups,
    };
    return mockImportPreview;
  },

  [RustCommand.ImportCommit]: ({ request }: Args): ImportResult => {
    const typed = request as ImportCommitRequest;
    const preview = mockImportPreview;
    if (!preview || preview.token !== typed.token) {
      throw 'Invalid state: A prévia da importação expirou — leia as conexões de novo';
    }
    mockImportPreview = null;

    const dataset = getDataset();
    const groupsBefore = dataset.groups.length;
    const groupIdOf = (name: string) => {
      const existing = dataset.groups.find(
        group => group.name.toLowerCase() === name.toLowerCase(),
      );
      if (existing) return existing.id;
      return (
        serverHandlers[RustCommand.CreateServerGroup]({ name }) as ServerGroup
      ).id;
    };

    let imported = 0;
    let withoutPassword = 0;
    for (const candidate of preview.candidates) {
      if (!typed.keys.includes(candidate.key) || !candidate.dbType) continue;
      const groupId =
        typed.keepGroups && candidate.group
          ? groupIdOf(candidate.group)
          : typed.targetGroupId;
      const hasPassword =
        typed.includePasswords && (candidate.hasPassword || candidate.passwordInKeychain);
      serverHandlers[RustCommand.CreateServer]({
        input: {
          name: candidate.name,
          dbType: candidate.dbType,
          host: candidate.host,
          port: candidate.port,
          username: candidate.username,
          password: hasPassword ? 'mock' : '',
          defaultDatabase: candidate.defaultDatabase,
          sslEnabled: candidate.sslEnabled,
          connectionUri: candidate.connectionUri,
          groupId,
        } satisfies ServerInput,
      });
      imported++;
      const needsPassword =
        candidate.dbType !== 'sqlite' && !candidate.connectionUri && !!candidate.username;
      if (needsPassword && !hasPassword) withoutPassword++;
    }
    return {
      imported,
      withoutPassword,
      keychainDenied: false,
      groupsCreated: dataset.groups.length - groupsBefore,
    };
  },

  [RustCommand.ImportDiscard]: (): void => {
    mockImportPreview = null;
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

/**
 * Tamanho em disco sintético, estável entre chamadas. Como no backend, só o
 * Postgres (e o Mongo, na estrutura da árvore) informam, e views ficam sem.
 */
const mockTableSize = (
  entry: MockServerEntry,
  table: MockTable,
  { includeMongo }: { includeMongo: boolean },
): number | null => {
  const { dbType } = entry.server;
  const supported =
    dbType === 'postgres' || (includeMongo && dbType === 'mongodb');
  if (!supported || table.tableType === 'view') return null;
  return 16_384 + table.rowEstimate * table.columns.length * 48;
};

const toTableInfo = (entry: MockServerEntry, table: MockTable): TableInfo => ({
  name: table.name,
  schema: table.schema,
  tableType: table.tableType,
  rowEstimate: table.rowEstimate,
  sizeBytes: mockTableSize(entry, table, { includeMongo: false }),
});

const structureHandlers: Record<string, MockHandler> = {
  [RustCommand.ListDatabases]: ({ serverId }: Args): DatabaseInfo[] => {
    const entry = requireServerEntry(serverId as number);
    if (isEmptyMode()) return [];
    // O Mongo lista só os nomes (`nameOnly`), como o backend; o escopo salvo
    // filtra os databases
    const withSize = entry.server.dbType !== 'mongodb';
    const allows = parseScope(entry.server.scopeDatabases);
    return entry.databases
      .filter(db => allows(db.name))
      .map(db => ({
        name: db.name,
        sizeBytes: withSize ? db.sizeBytes : null,
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
      .map(table => toTableInfo(entry, table));
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
          .map(table => ({
            name: table.name,
            tableType: table.tableType,
            sizeBytes: mockTableSize(entry, table, { includeMongo: true }),
          })),
      })),
      fetchedAt,
    };
  },
};

// ── Edição da estrutura (DDL) ───────────────────────────────────────────────

/** Como no backend: só Postgres e SQLite editam a estrutura pela árvore */
const requireStructureEditing = (serverId: unknown) => {
  const entry = requireServerEntry(serverId as number);
  const { dbType } = entry.server;
  if (dbType !== 'postgres' && dbType !== 'sqlite') {
    throw 'Unsupported type: Editing the structure is not supported for this database';
  }
  return entry;
};

const ddlHandlers: Record<string, MockHandler> = {
  [RustCommand.PreviewStructureChange]: ({ serverId, change }: Args) =>
    structureSql(
      requireStructureEditing(serverId).server.dbType,
      change as StructureChange,
    ),

  [RustCommand.ApplyStructureChange]: ({
    serverId,
    database,
    change,
  }: Args): StatementResult => {
    requireStructureEditing(serverId);
    applyStructure(serverId as number, database as string, change as StructureChange);
    return { affectedRows: 0, executionTimeMs: 12 };
  },
};

// ── Catálogo de metadados ───────────────────────────────────────────────────

const catalogHandlers: Record<string, MockHandler> = {
  [RustCommand.CatalogOpen]: args => mockCatalog.open(args),
  [RustCommand.CatalogStatus]: args => mockCatalog.status(args),
  [RustCommand.CatalogChildren]: args => mockCatalog.children(args),
  [RustCommand.CatalogSearch]: args => mockCatalog.search(args),
  [RustCommand.CatalogResolve]: args => mockCatalog.resolve(args),
  [RustCommand.CatalogComplete]: args => mockCatalog.complete(args),
  [RustCommand.CatalogDrift]: args => mockCatalog.drift(args),
  [RustCommand.CatalogCompareSchemas]: args => mockCatalog.compareSchemas(args),
  [RustCommand.CatalogRefresh]: args => mockCatalog.refresh(args),
  // A sincronização simulada termina sozinha: não há o que interromper
  [RustCommand.CatalogCancel]: () => false,
  [RustCommand.CatalogRelationSize]: args => mockCatalog.relationSize(args),
  [RustCommand.CatalogShapes]: args => mockCatalog.shapes(args),
  [RustCommand.CatalogDiagnostics]: args => mockCatalog.diagnostics(args),
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

// ── Preferências do app ─────────────────────────────────────────────────────

const SETTINGS_STORAGE_KEY = 'octapus-mock-settings';
const DEFAULT_SETTINGS: AppSettings = { betaUpdates: false, pgToolsDir: null };

const settingsHandlers: Record<string, MockHandler> = {
  // Mesma regra do Rust: o que nunca foi salvo volta com o padrão
  [RustCommand.GetSettings]: (): AppSettings => {
    try {
      const saved = JSON.parse(
        localStorage.getItem(SETTINGS_STORAGE_KEY) ?? '{}',
      );
      return { ...DEFAULT_SETTINGS, ...saved };
    } catch {
      return DEFAULT_SETTINGS;
    }
  },

  [RustCommand.UpdateSettings]: ({ settings }: Args): AppSettings => {
    localStorage.setItem(SETTINGS_STORAGE_KEY, JSON.stringify(settings));
    return settings as AppSettings;
  },
};

// ── Ferramentas do Postgres (backup/restore) ────────────────────────────────

/** Uma instalação falsa do Homebrew, como o backend acharia */
const mockPgInstall = (
  dir: string,
  version: string,
  versionNum: number,
  source: PgToolsInstall['source'],
): PgToolsInstall => ({
  dir,
  source,
  version,
  versionNum,
  pgDump: `${dir}/pg_dump`,
  pgRestore: `${dir}/pg_restore`,
  psql: `${dir}/psql`,
});

const MOCK_PG_CANDIDATES: PgToolsInstall[] = [
  mockPgInstall('/opt/homebrew/opt/postgresql@17/bin', '17.2', 170_002, 'knownLocation'),
  mockPgInstall('/opt/homebrew/opt/postgresql@15/bin', '15.10', 150_010, 'knownLocation'),
];

/**
 * Mesma regra do Rust: a pasta configurada vale sozinha (uma com "invalida"
 * no caminho simula a pasta sem pg_dump); sem ela, a mais nova achada.
 */
function mockPgDetection(): PgToolsDetection {
  const { pgToolsDir } = settingsHandlers[RustCommand.GetSettings]({}) as AppSettings;
  if (!pgToolsDir) {
    return {
      selected: MOCK_PG_CANDIDATES[0],
      candidates: MOCK_PG_CANDIDATES,
      configuredError: null,
    };
  }
  if (pgToolsDir.includes('invalida')) {
    return {
      selected: null,
      candidates: MOCK_PG_CANDIDATES,
      configuredError: `Não há pg_dump em ${pgToolsDir}.`,
    };
  }
  return {
    selected: mockPgInstall(pgToolsDir, '16.4', 160_004, 'configured'),
    candidates: MOCK_PG_CANDIDATES,
    configuredError: null,
  };
}

const pgToolsHandlers: Record<string, MockHandler> = {
  [RustCommand.PgToolsDetect]: (): PgToolsDetection => mockPgDetection(),

  [RustCommand.PgToolsStatus]: ({ serverId }: Args): PgToolsStatus => {
    const entry = requireServerEntry(serverId as number);
    if (entry.server.dbType !== 'postgres') {
      throw 'Backup e restauração só existem para PostgreSQL.';
    }
    const detection = mockPgDetection();
    const serverVersion = '16.4';
    const serverVersionNum = 160_004;
    const major = (num: number) =>
      num >= 100_000 ? Math.floor(num / 10_000) : Math.floor(num / 100);
    const problem: PgToolsStatus['problem'] = detection.configuredError
      ? { kind: 'invalidConfigured', message: detection.configuredError }
      : !detection.selected
        ? { kind: 'notFound' }
        : major(detection.selected.versionNum) < major(serverVersionNum)
          ? {
              kind: 'clientTooOld',
              client: detection.selected.version,
              server: serverVersion,
            }
          : null;
    return { ...detection, serverVersion, serverVersionNum, problem };
  },
};

// ── Backup (pg_dump) ────────────────────────────────────────────────────────

/** Jobs falsos em andamento → cancelar */
const mockBackupJobs = new Map<string, () => void>();
let mockBackupSeq = 0;

const emitBackup = (event: BackupEvent) => mockEmit(BACKUP_EVENT, event);

/** A linha de comando como o Rust monta (args.rs), sem a senha */
function mockDumpCommand(request: BackupRequest, file: string): string {
  const { target, options } = request;
  const entry = requireServerEntry(request.serverId);
  const quote = (name: string) => `"${name.replace(/"/g, '""')}"`;
  const args = [
    '--verbose',
    '--no-password',
    `--format=${options.format}`,
    `--file=${file}`,
    ...(options.content === 'schemaOnly' ? ['--schema-only'] : []),
    ...(options.content === 'dataOnly' ? ['--data-only'] : []),
    ...(options.noOwner ? ['--no-owner', '--no-privileges'] : []),
    ...(options.clean ? ['--clean', '--if-exists'] : []),
    ...(options.columnInserts ? ['--column-inserts'] : []),
    ...(target.kind === 'schema' ? [`--schema=${quote(target.schema)}`] : []),
    ...(target.kind === 'table'
      ? [`--table=${quote(target.schema)}.${quote(target.table)}`]
      : []),
    `--host=${entry.server.host}`,
    `--port=${entry.server.port}`,
    `--username=${entry.server.username}`,
    `--dbname=${request.database}`,
  ];
  const shell = (arg: string) =>
    /^[\w=./:,@%+-]+$/.test(arg) ? arg : `'${arg.replace(/'/g, `'\\''`)}'`;
  return ['pg_dump', ...args.map(shell)].join(' ');
}

const backupHandlers: Record<string, MockHandler> = {
  [RustCommand.BackupPreview]: ({ request }: Args) => {
    const backup = request as BackupRequest;
    const { target, options } = backup;
    // Hora local, como o `chrono::Local` do backend
    const now = new Date();
    const pad = (n: number) => String(n).padStart(2, '0');
    const stamp = `${now.getFullYear()}${pad(now.getMonth() + 1)}${pad(now.getDate())}_${pad(now.getHours())}${pad(now.getMinutes())}`;
    const parts = [
      backup.database,
      ...(target.kind === 'database' ? [] : [target.schema]),
      ...(target.kind === 'table' ? [target.table] : []),
    ].map(part => part.replace(/[^\w\u00C0-\u024F-]/g, '_'));
    const fileName = `${parts.join('_')}_${stamp}.${options.format === 'custom' ? 'dump' : 'sql'}`;
    return { command: mockDumpCommand(backup, fileName), fileName };
  },

  /**
   * Um pg_dump de mentira: algumas linhas de log por meio segundo e o fim.
   * Um caminho com "erro" simula a falha de autenticação.
   */
  [RustCommand.BackupStart]: ({ request, path }: Args) => {
    const backup = request as BackupRequest;
    const file = path as string;
    const jobId = `job-${++mockBackupSeq}`;
    const subject =
      backup.target.kind === 'table'
        ? `${backup.target.schema}.${backup.target.table}`
        : backup.target.kind === 'schema'
          ? backup.target.schema
          : backup.database;
    const steps = [
      'pg_dump: last built-in OID is 16383',
      'pg_dump: reading extensions',
      'pg_dump: reading schemas',
      'pg_dump: reading user-defined tables',
      `pg_dump: dumping contents of ${subject}`,
      'pg_dump: saving encoding = UTF8',
    ];
    const timers: number[] = [];
    let step = 0;
    const tick = () => {
      if (step < steps.length) {
        emitBackup({ jobId, type: 'log', lines: [steps[step++]] });
        timers.push(window.setTimeout(tick, 500));
        return;
      }
      mockBackupJobs.delete(jobId);
      emitBackup(
        file.includes('erro')
          ? {
              jobId,
              type: 'error',
              message:
                'pg_dump: error: connection to server failed: FATAL:  password authentication failed for user "app"',
            }
          : {
              jobId,
              type: 'done',
              path: file,
              sizeBytes: 3_456_789,
              elapsedMs: steps.length * 500,
              warnings: [],
            },
      );
    };
    timers.push(window.setTimeout(tick, 300));
    mockBackupJobs.set(jobId, () => {
      timers.forEach(timer => window.clearTimeout(timer));
      emitBackup({ jobId, type: 'cancelled' });
    });
    return jobId;
  },

  /**
   * `.sql` é script; com "invalido" no caminho, um arquivo que não é backup;
   * o resto é um .dump com o resumo de sempre.
   */
  [RustCommand.RestoreInspect]: ({ path }: Args): RestoreInspection => {
    const file = path as string;
    if (file.includes('invalido')) {
      throw 'Não parece um backup do PostgreSQL (.dump) nem um script SQL.';
    }
    if (file.endsWith('.sql')) {
      return { path: file, format: 'plain', sizeBytes: 812_345, summary: null, summaryError: null };
    }
    return {
      path: file,
      format: 'custom',
      sizeBytes: 3_456_789,
      summary: {
        database: 'loja',
        serverVersion: '16.4 (Debian 16.4-1.pgdg120+1)',
        pgDumpVersion: '17.2',
        schemas: ['public', 'vendas'],
        schemaCount: 2,
        tableCount: 7,
        viewCount: 1,
        hasData: true,
      },
      summaryError: null,
    };
  },

  [RustCommand.RestorePreview]: ({ request, path }: Args) => {
    const restore = request as RestoreRequest;
    const entry = requireServerEntry(restore.serverId);
    const { options } = restore;
    const connection = [
      `--host=${entry.server.host}`,
      `--port=${entry.server.port}`,
      `--username=${entry.server.username}`,
      `--dbname=${restore.database}`,
    ];
    if (restore.format === 'plain') {
      return [
        'psql --no-psqlrc --no-password',
        ...(options.singleTransaction ? ['--set=ON_ERROR_STOP=1 --single-transaction'] : []),
        `--file=${path as string}`,
        ...connection,
      ].join(' ');
    }
    return [
      'pg_restore --verbose --no-password',
      ...(options.noOwner ? ['--no-owner --no-privileges'] : []),
      ...(options.clean ? ['--clean --if-exists'] : []),
      ...(options.singleTransaction ? ['--single-transaction --exit-on-error'] : []),
      ...connection,
      path as string,
    ].join(' ');
  },

  /** Restauração de mentira: sem "tudo ou nada", termina com um erro ignorado */
  [RustCommand.RestoreStart]: ({ request, path }: Args) => {
    const restore = request as RestoreRequest;
    const jobId = `job-${++mockBackupSeq}`;
    const steps = [
      'pg_restore: connecting to database for restore',
      'pg_restore: creating SCHEMA "vendas"',
      'pg_restore: creating TABLE "public.pedidos"',
      'pg_restore: processing data for table "public.pedidos"',
      'pg_restore: creating CONSTRAINT "public.pedidos pedidos_pkey"',
    ];
    const timers: number[] = [];
    let step = 0;
    const tick = () => {
      if (step < steps.length) {
        emitBackup({ jobId, type: 'log', lines: [steps[step++]] });
        timers.push(window.setTimeout(tick, 500));
        return;
      }
      mockBackupJobs.delete(jobId);
      emitBackup(
        (path as string).includes('erro')
          ? {
              jobId,
              type: 'error',
              message: 'pg_restore: error: could not execute query: ERROR:  schema "vendas" already exists',
            }
          : {
              jobId,
              type: 'done',
              path: '',
              sizeBytes: 0,
              elapsedMs: steps.length * 500,
              warnings: restore.options.singleTransaction
                ? []
                : ['pg_restore: error: could not execute query: ERROR:  role "app" does not exist'],
            },
      );
    };
    timers.push(window.setTimeout(tick, 300));
    mockBackupJobs.set(jobId, () => {
      timers.forEach(timer => window.clearTimeout(timer));
      emitBackup({ jobId, type: 'cancelled' });
    });
    return jobId;
  },

  [RustCommand.BackupCancel]: ({ jobId }: Args) => {
    const cancel = mockBackupJobs.get(jobId as string);
    mockBackupJobs.delete(jobId as string);
    cancel?.();
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

let mockWindowMaximized = false;

const pluginHandlers: Record<string, MockHandler> = {
  'plugin:updater|check': () => {
    if (!useMockStore.getState().updateAvailable) return null;
    return {
      rid: 1,
      available: true,
      currentVersion: '0.1.0-beta.16',
      version: '1.0.0',
      date: '2026-09-14 12:00:00.000 +00:00:00',
      body: 'Notas de versão simuladas pelo modo mock.',
      rawJson: {},
    };
  },

  // Canal beta: o mesmo update fake, numa pre-release
  [RustCommand.CheckBetaUpdate]: () => {
    if (!useMockStore.getState().updateAvailable) return null;
    return {
      rid: 1,
      currentVersion: '0.1.0-beta.16',
      version: '0.2.0-beta.1',
      body: 'Pre-release simulada pelo modo mock.',
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
  // Escolha de pasta (ferramentas do Postgres): a do Homebrew; de arquivo
  // (restauração): um .dump de exemplo
  'plugin:dialog|open': ({ options }: Args) => {
    const { directory, filters } = (options ?? {}) as {
      directory?: boolean;
      filters?: { extensions: string[] }[];
    };
    if (directory) return '/opt/homebrew/opt/postgresql@16/bin';
    // Importar conexões: o arquivo de export do app, pela extensão do filtro
    const extension = filters?.[0]?.extensions[0];
    if (extension && extension !== 'dump') {
      return `/Users/voce/Downloads/conexoes.${extension}`;
    }
    return '/Users/voce/Downloads/loja_20261015_1432.dump';
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

  // Barra de título: não há janela nativa, só o estado que o front consulta.
  'plugin:window|minimize': () => null,
  'plugin:window|toggle_maximize': () => {
    mockWindowMaximized = !mockWindowMaximized;
  },
  'plugin:window|is_maximized': () => mockWindowMaximized,
  'plugin:window|close': () => {
    console.info('[octapus-mock] fechar janela ignorado');
  },
  'plugin:window|is_fullscreen': () => false,
  'plugin:window|set_fullscreen': () => null,
  // Eventos globais: o catálogo do mock emite `catalog-event` (ver events.ts);
  // o `onResized` também assina, mas nunca dispara.
  'plugin:event|listen': ({ event, handler }: Args) =>
    registerListener(event as string, handler as number),
  'plugin:event|unlisten': ({ event, eventId }: Args) =>
    unregisterListener(event as string, eventId as number),

  // Fora do Tauri não há navegador do sistema: abre numa aba nova.
  'plugin:opener|open_url': ({ url }: Args) => {
    window.open(url as string, '_blank', 'noopener');
  },
  // Sem gerenciador de arquivos: só registra o pedido
  'plugin:opener|reveal_item_in_dir': ({ path }: Args) => {
    console.info('[octapus-mock] mostrar na pasta', path);
  },
};

export const handlers: Record<string, MockHandler> = {
  ...serverHandlers,
  ...importHandlers,
  ...connectionHandlers,
  ...structureHandlers,
  ...ddlHandlers,
  ...catalogHandlers,
  ...browseHandlers,
  ...exportHandlers,
  ...sessionHandlers,
  ...settingsHandlers,
  ...pgToolsHandlers,
  ...backupHandlers,
  ...queryHandlers,
  ...pluginHandlers,
};
