import { CompletionContext, type Completion } from '@codemirror/autocomplete';
import { PostgreSQL, sql } from '@codemirror/lang-sql';
import { EditorState } from '@codemirror/state';
import { describe, expect, it, vi } from 'vitest';
import type { DatabaseStructure } from '@/api/types/structure.types';
import type { SqlCompletionPorts } from './sql-completion.types';
import { createSqlSchemaSource, warmNeeds } from './sql-schema-source';

/** "Banco" com catálogo: 120 tenants iguais + public */
const TENANTS = Array.from(
  { length: 120 },
  (_, i) => `tenant_${String(i + 1).padStart(5, '0')}`,
);
const TABLES: Record<string, string[]> = Object.fromEntries([
  ['public', ['plans']],
  ...TENANTS.map(schema => [schema, ['customers', 'orders']]),
]);

function catalogPorts() {
  let structure: DatabaseStructure = { schemas: [], fetchedAt: 0 };
  const ports: SqlCompletionPorts = {
    getStructure: () => structure,
    getDefaultSchema: () => null,
    peekColumns: () => undefined,
    ensureColumns: async () => undefined,
    getColumnsVersion: () => 0,
    warm: vi.fn(async ({ schemas, tables }) => {
      const wanted = new Set(['public', ...schemas]);
      // Sem schema: o "catálogo" resolve para o primeiro schema que tem a tabela
      for (const table of tables) {
        const owner = Object.keys(TABLES).find(schema =>
          TABLES[schema].includes(table),
        );
        if (owner) wanted.add(owner);
      }
      structure = {
        schemas: Array.from(wanted)
          .filter(schema => TABLES[schema])
          .map(name => ({
            name,
            tables: TABLES[name].map(table => ({
              name: table,
              tableType: 'table' as const,
              sizeBytes: null,
            })),
          })),
        fetchedAt: 1,
      };
    }),
    completeSchemas: vi.fn(async prefix =>
      Object.keys(TABLES)
        .filter(schema => schema.startsWith(prefix))
        .slice(0, 50),
    ),
  };
  return ports;
}

function complete(ports: SqlCompletionPorts, doc: string, pos = doc.length) {
  const state = EditorState.create({
    doc,
    extensions: [sql({ dialect: PostgreSQL })],
  });
  return createSqlSchemaSource(ports)(new CompletionContext(state, pos, true));
}

const labels = (options: readonly Completion[] | undefined) =>
  (options ?? []).map(option => option.label);

describe('createSqlSchemaSource com catálogo', () => {
  it('aquece o schema antes do ponto e lista as relações dele', async () => {
    const ports = catalogPorts();
    const result = await complete(ports, 'SELECT * FROM tenant_00042.');

    expect(ports.warm).toHaveBeenCalledWith({
      schemas: ['tenant_00042'],
      tables: [],
    });
    expect(labels(result?.options)).toEqual(
      expect.arrayContaining(['customers', 'orders']),
    );
  });

  it('completa schemas fora do namespace por prefixo', async () => {
    const ports = catalogPorts();
    const result = await complete(ports, 'SELECT * FROM tenant_0004');

    expect(ports.completeSchemas).toHaveBeenCalledWith('tenant_0004');
    const schemas =
      result?.options.filter(option => option.type === 'namespace') ?? [];
    expect(schemas.map(option => option.label)).toEqual(
      Array.from({ length: 10 }, (_, i) => `tenant_0004${i}`),
    );
    expect(result?.validFor).toBeDefined();
  });

  it('lista cortada no teto refaz a busca na próxima tecla', async () => {
    const result = await complete(catalogPorts(), 'SELECT * FROM tenant');
    expect(
      result?.options.filter(option => option.type === 'namespace'),
    ).toHaveLength(50);
    expect(result?.validFor).toBeUndefined();
  });

  it('prefixo de uma letra não lista milhares de schemas', async () => {
    const ports = catalogPorts();
    await complete(ports, 'SELECT * FROM t');
    expect(ports.completeSchemas).not.toHaveBeenCalled();
  });

  it('qualificador que é tabela quente não vira schema', async () => {
    const ports = catalogPorts();
    // Primeiro aquecimento traz `orders` (resolvido sem schema)
    await complete(ports, 'SELECT * FROM orders');
    const doc = 'SELECT orders. FROM orders';
    await complete(ports, doc, 'SELECT orders.'.length);
    expect(ports.warm).toHaveBeenLastCalledWith({
      schemas: [],
      tables: ['orders'],
    });
  });

  it('alias do statement não é tratado como schema', async () => {
    const ports = catalogPorts();
    const doc = 'SELECT o. FROM orders o';
    await complete(ports, doc, 'SELECT o.'.length);
    expect(ports.warm).toHaveBeenCalledWith({
      schemas: [],
      tables: ['orders'],
    });
  });
});

describe('createSqlSchemaSource sem schema (SQLite)', () => {
  /** Como o `useSqlCompletion` monta: o banco é o schema padrão */
  function flatPorts(): SqlCompletionPorts {
    const structure: DatabaseStructure = {
      schemas: [
        {
          name: 'main',
          tables: ['notas', 'tags'].map(name => ({
            name,
            tableType: 'table' as const,
            sizeBytes: null,
          })),
        },
      ],
      fetchedAt: 1,
    };
    return {
      getStructure: () => structure,
      getDefaultSchema: () => 'main',
      peekColumns: () => undefined,
      ensureColumns: async () => undefined,
      getColumnsVersion: () => 0,
      warm: vi.fn(async () => undefined),
      completeSchemas: vi.fn(async () => []),
    };
  }

  it('tabelas no nível de cima, sem precisar do schema', async () => {
    const result = await complete(flatPorts(), 'SELECT * FROM ');
    expect(labels(result?.options)).toEqual(
      expect.arrayContaining(['notas', 'tags']),
    );
  });

  it('`main.` também completa', async () => {
    const result = await complete(flatPorts(), 'SELECT * FROM main.');
    expect(labels(result?.options)).toEqual(
      expect.arrayContaining(['notas', 'tags']),
    );
  });
});

describe('createSqlSchemaSource com colunas provisórias', () => {
  const ORDERS_COLUMNS = ['id', 'total'].map((name, i) => ({
    name,
    ordinal: i + 1,
    dataType: i === 0 ? 'integer' : 'numeric',
    isNullable: false,
    defaultValue: null,
    isPrimaryKey: i === 0,
    isForeignKey: false,
  }));

  it('outro tenant empresta as colunas na hora; as de verdade vêm depois', async () => {
    const ports = catalogPorts();
    // As colunas deste tenant nunca chegam durante o teste: se a source
    // esperasse por elas, a sugestão não sairia
    ports.ensureColumns = vi.fn(() => new Promise<undefined>(() => {}));
    ports.peekSimilarColumns = vi.fn((schema: string, table: string) =>
      schema !== 'tenant_00001' && table === 'orders'
        ? { columns: ORDERS_COLUMNS, from: 'tenant_00001' }
        : undefined,
    );

    const result = await complete(
      ports,
      'SELECT * FROM tenant_00042.orders o WHERE o.',
    );
    const options = result?.options ?? [];
    expect(options.map(option => option.label)).toEqual(
      expect.arrayContaining(['id', 'total']),
    );
    expect(options.find(option => option.label === 'id')?.detail).toContain(
      'prévia de tenant_00001',
    );
    expect(ports.ensureColumns).toHaveBeenCalledWith('tenant_00042', 'orders');
  });
});

describe('warmNeeds', () => {
  it('separa schemas citados de tabelas sem schema', () => {
    expect(
      warmNeeds(
        [
          { schemaHint: 'tenant_00001', table: 'orders' },
          { table: 'customers', alias: 'c' },
        ],
        'tenant_00002',
      ),
    ).toEqual({
      schemas: ['tenant_00001', 'tenant_00002'],
      tables: ['customers'],
    });
  });

  it('qualificador igual a alias ou tabela fica de fora', () => {
    expect(warmNeeds([{ table: 'orders', alias: 'o' }], 'O').schemas).toEqual(
      [],
    );
    expect(warmNeeds([{ table: 'orders' }], 'orders').schemas).toEqual([]);
  });
});
