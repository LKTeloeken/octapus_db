import type { MockColumn, MockDatabase, MockTable } from './types';
import * as gen from './rows';

/**
 * Database multi-tenant do mock: um schema por cliente, as mesmas 150 tabelas
 * em cada — o mesmo molde da fixture de desempenho (`perf/catalog/`). Serve
 * para desenhar e conferir a árvore, a palette e o autocomplete em escala sem
 * subir banco nenhum.
 *
 * Nada é materializado de antemão (750 mil tabelas): o catálogo do mock lê a
 * forma daqui, e uma tabela só vira `MockTable` quando alguém a abre.
 */

export const SAAS_DATABASE = 'saas';
export const TENANT_COUNT = 5_000;

const ENTITIES = [
  'customers',
  'orders',
  'invoices',
  'payments',
  'products',
  'categories',
  'suppliers',
  'shipments',
  'addresses',
  'contacts',
  'users',
  'roles',
  'permissions',
  'sessions',
  'notifications',
  'tickets',
  'comments',
  'attachments',
  'tags',
  'projects',
  'tasks',
  'events',
  'webhooks',
  'settings',
  'reports',
];
const SUFFIXES = ['', '_items', '_history', '_audit', '_settings', '_links'];

/** As 150 tabelas de um tenant, em ordem de criação */
const TENANT_TABLES: readonly string[] = Array.from(
  { length: ENTITIES.length * SUFFIXES.length },
  (_, index) => ENTITIES[index % 25] + SUFFIXES[Math.floor(index / 25)],
);

/** Tabelas compartilhadas, fora dos tenants */
export const SAAS_PUBLIC_TABLES = [
  'plans',
  'tenant_registry',
  'schema_migrations',
];

export function tenantSchema(index: number): string {
  return `tenant_${String(index).padStart(5, '0')}`;
}

export function tenantIndex(schema: string): number | null {
  const match = /^tenant_(\d{5})$/.exec(schema);
  if (!match) return null;
  const index = Number(match[1]);
  return index >= 1 && index <= TENANT_COUNT ? index : null;
}

/** Como na fixture: tenants múltiplos de 100 estão sem as 3 últimas tabelas */
export function tenantTables(index: number): readonly string[] {
  return index % 100 === 0 ? TENANT_TABLES.slice(0, -3) : TENANT_TABLES;
}

const column = (
  name: string,
  dataType: string,
  typeOid: number,
  cell: gen.CellGen,
  overrides: Partial<MockColumn> = {},
): MockColumn => ({
  name,
  dataType,
  typeOid,
  isNullable: true,
  defaultValue: null,
  isPrimaryKey: false,
  isForeignKey: false,
  gen: cell,
  ...overrides,
});

const TENANT_COLUMNS: MockColumn[] = [
  column('id', 'integer', 23, gen.serial(), {
    isNullable: false,
    isPrimaryKey: true,
    defaultValue: "nextval('id_seq'::regclass)",
  }),
  column('tenant_ref', 'integer', 23, gen.int(1, 9_999), { isNullable: false }),
  column(
    'name',
    'text',
    25,
    gen.pick(['alfa', 'beta', 'gama', 'delta', 'ômega']),
  ),
  column('status', 'text', 25, gen.pick(['ativo', 'pendente', 'arquivado'])),
];

const materialized = new Map<string, MockTable>();

/** A tabela de um tenant, criada no primeiro acesso; `undefined` se não existe */
export function tenantTable(
  schema: string,
  name: string,
): MockTable | undefined {
  const known =
    schema === 'public'
      ? SAAS_PUBLIC_TABLES.includes(name)
      : (() => {
          const index = tenantIndex(schema);
          return index !== null && tenantTables(index).includes(name);
        })();
  if (!known) return undefined;

  const key = `${schema}.${name}`;
  let table = materialized.get(key);
  if (!table) {
    table = {
      name,
      schema,
      tableType: 'table',
      columns: TENANT_COLUMNS,
      indexes: [],
      rowEstimate: 40,
    };
    materialized.set(key, table);
  }
  return table;
}

export function buildSaasDatabase(): MockDatabase {
  return {
    name: SAAS_DATABASE,
    sizeBytes: null,
    tables: [],
    generated: 'tenants',
  };
}
