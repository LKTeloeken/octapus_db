import type { DatabaseStructure } from '@/api/types/structure.types';

/**
 * Estruturas sintéticas no mesmo formato da fixture do Postgres
 * (`perf/catalog/seed/pg-tenants.sql`): um schema por cliente com as mesmas
 * tabelas. O tier `U` é o pior caso sem tenants — nomes todos diferentes, onde
 * deduplicar por formato de schema não ajuda em nada.
 */
export interface StructureTier {
  label: string;
  schemas: number;
  tablesPerSchema: number;
  uniqueNames: boolean;
}

export const STRUCTURE_TIERS: Record<string, StructureTier> = {
  S: { label: 'S', schemas: 500, tablesPerSchema: 150, uniqueNames: false },
  M: { label: 'M', schemas: 2000, tablesPerSchema: 150, uniqueNames: false },
  L: { label: 'L', schemas: 5000, tablesPerSchema: 150, uniqueNames: false },
  U: { label: 'U', schemas: 1500, tablesPerSchema: 500, uniqueNames: true },
};

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

/** Mesmo nome que `perf_table_name(t)` gera no seed SQL (t começa em 1). */
export function tableName(t: number): string {
  const index = t - 1;
  return (
    ENTITIES[index % 25] + SUFFIXES[Math.floor(index / 25) % SUFFIXES.length]
  );
}

export function schemaName(i: number): string {
  return `tenant_${String(i).padStart(5, '0')}`;
}

export function makeStructure(tier: StructureTier): DatabaseStructure {
  const schemas: DatabaseStructure['schemas'] = [];

  for (let i = 1; i <= tier.schemas; i++) {
    const tables: DatabaseStructure['schemas'][number]['tables'] = [];

    for (let t = 1; t <= tier.tablesPerSchema; t++) {
      tables.push({
        name: tier.uniqueNames ? `${tableName(t)}_${i}_${t}` : tableName(t),
        tableType: 'table',
        sizeBytes: 16384,
      });
    }

    schemas.push({ name: schemaName(i), tables });
  }

  return { schemas, fetchedAt: Date.now() };
}
