import type { AdapterCapabilities } from '@/api/types/capabilities.types';
import type { DatabaseType, Server } from '@/api/types/server.types';
import { generateRows, seedFrom } from './rows';
import { buildMongoDatabases } from './mongo';
import { buildPostgresDatabases } from './postgres';
import { buildRedisDatabases } from './redis';
import type { MockDatabase, MockServerEntry, MockTable } from './types';

export * from './types';

/**
 * Presets fixos por tipo de banco, iguais aos de
 * `src-tauri/src/models/capabilities.rs` — `get_capabilities` é síncrono e não
 * abre conexão, então o valor não depende de nada.
 */
export const CAPABILITIES: Record<DatabaseType, AdapterCapabilities> = {
  postgres: {
    hasSchemas: true,
    hasPrimaryKeys: true,
    supportsSql: true,
    supportsTransactions: true,
    supportsIndexes: true,
    browsable: true,
  },
  mongodb: {
    hasSchemas: false,
    hasPrimaryKeys: true,
    supportsSql: false,
    supportsTransactions: false,
    supportsIndexes: true,
    browsable: true,
  },
  redis: {
    hasSchemas: false,
    hasPrimaryKeys: false,
    supportsSql: false,
    supportsTransactions: false,
    supportsIndexes: false,
    browsable: true,
  },
};

export function buildDatabasesFor(dbType: DatabaseType): MockDatabase[] {
  switch (dbType) {
    case 'postgres':
      return buildPostgresDatabases();
    case 'mongodb':
      return buildMongoDatabases();
    case 'redis':
      return buildRedisDatabases();
  }
}

export interface MockDataset {
  servers: MockServerEntry[];
  nextServerId: number;
}

const SEED_SERVERS: Omit<Server, 'id'>[] = [
  {
    name: 'Loja (local)',
    dbType: 'postgres',
    host: 'localhost',
    port: 5432,
    username: 'postgres',
    defaultDatabase: 'loja',
    sslEnabled: false,
    connectionUri: null,
    createdAt: 1_735_689_600,
  },
  {
    name: 'Catálogo (Atlas)',
    dbType: 'mongodb',
    host: 'cluster0.exemplo.mongodb.net',
    port: 27017,
    username: 'app',
    defaultDatabase: 'catalogo',
    sslEnabled: true,
    connectionUri: 'mongodb+srv://cluster0.exemplo.mongodb.net',
    createdAt: 1_743_465_600,
  },
  {
    name: 'Cache (dev)',
    dbType: 'redis',
    host: '127.0.0.1',
    port: 6379,
    username: 'default',
    defaultDatabase: '0',
    sslEnabled: false,
    connectionUri: null,
    createdAt: 1_751_328_000,
  },
];

function buildDataset(): MockDataset {
  const servers = SEED_SERVERS.map((seed, index) => ({
    server: { ...seed, id: index + 1 },
    capabilities: CAPABILITIES[seed.dbType],
    databases: buildDatabasesFor(seed.dbType),
  }));

  return { servers, nextServerId: servers.length + 1 };
}

let dataset: MockDataset | null = null;

export function getDataset(): MockDataset {
  dataset ??= buildDataset();
  return dataset;
}

/** Descarta toda mutação em memória e volta ao estado inicial */
export function resetDataset(): void {
  dataset = null;
}

// ── Lookups ─────────────────────────────────────────────────────────────────

export function findServerEntry(serverId: number): MockServerEntry | undefined {
  return getDataset().servers.find(entry => entry.server.id === serverId);
}

/** Rejeita com a mesma string do backend (`Error::NotFound`) */
export function requireServerEntry(serverId: number): MockServerEntry {
  const entry = findServerEntry(serverId);
  if (!entry) throw `Not found: Server with id ${serverId} not found`;
  return entry;
}

export function requireDatabase(
  entry: MockServerEntry,
  database: string,
): MockDatabase {
  const found = entry.databases.find(db => db.name === database);
  if (!found) throw `Connection error: database "${database}" does not exist`;
  return found;
}

/**
 * `schema` vem null/'' nos adapters sem schema. Postgres cai no 'public'
 * quando o front não manda nada, igual ao default do `TableDataRequest`.
 */
export function requireTable(
  entry: MockServerEntry,
  database: string,
  schema: string | null | undefined,
  table: string,
): MockTable {
  const db = requireDatabase(entry, database);
  const wanted = entry.capabilities.hasSchemas ? schema || 'public' : '';
  const found = db.tables.find(
    candidate => candidate.name === table && candidate.schema === wanted,
  );
  if (!found)
    throw `Query error: relation "${qualify(wanted, table)}" does not exist`;
  return found;
}

export function qualify(schema: string, table: string): string {
  return schema ? `${schema}.${table}` : table;
}

// ── Materialização preguiçosa ───────────────────────────────────────────────

/**
 * As linhas só são geradas no primeiro acesso — o boot fica instantâneo mesmo
 * com tabelas de 20 mil linhas. A partir daí o array é o estado mutável que os
 * comandos de edição alteram.
 */
export function tableRows(table: MockTable): (string | null)[][] {
  if (!table.rows) {
    const seed = seedFrom(table.schema, table.name, table.rowEstimate);
    table.rows = generateRows(
      seed,
      table.rowEstimate,
      table.columns.map(column => column.gen),
    );
    table.nextSerial = table.rowEstimate + 1;
  }
  return table.rows;
}

/** Próximo id para a PK sequencial, usada nos inserts do mock */
export function takeSerial(table: MockTable): number {
  tableRows(table);
  const next = table.nextSerial ?? table.rowEstimate + 1;
  table.nextSerial = next + 1;
  return next;
}
