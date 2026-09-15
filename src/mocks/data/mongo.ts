import type { MockColumn, MockDatabase, MockTable } from './types';
import * as gen from './rows';

/**
 * No Mongo o adapter não tem catálogo: as "colunas" são inferidas dos
 * documentos e o `typeOid` é sempre null. O nome do tipo é o do BSON.
 */
const field = (
  name: string,
  bsonType: string,
  cell: gen.CellGen,
  overrides: Partial<Omit<MockColumn, 'name' | 'gen'>> = {},
): MockColumn => ({
  name,
  dataType: bsonType,
  typeOid: null,
  isNullable: true,
  defaultValue: null,
  isPrimaryKey: false,
  isForeignKey: false,
  gen: cell,
  ...overrides,
});

const objectIdField = (name = '_id'): MockColumn =>
  field(name, 'objectId', gen.objectId(), {
    isNullable: false,
    isPrimaryKey: true,
  });

const produtos: MockTable = {
  name: 'produtos',
  schema: '',
  tableType: 'table',
  rowEstimate: 2_400,
  columns: [
    objectIdField(),
    field('slug', 'string', gen.sku(), { isNullable: false }),
    field('titulo', 'string', gen.productName(), { isNullable: false }),
    field('preco', 'double', gen.decimal(9.9, 4_999)),
    field('estoque', 'int', gen.int(0, 900)),
    field(
      'categorias',
      'array',
      gen.textArray(['casa', 'escritorio', 'eletronicos', 'moda'], 1, 3),
    ),
    field('atributos', 'object', gen.json()),
    field('publicado', 'bool', gen.bool()),
    field('criadoEm', 'date', gen.timestamp(2024, 2026)),
  ],
  indexes: [
    {
      name: '_id_',
      columns: ['_id'],
      isUnique: true,
      isPrimary: true,
      indexType: 'btree',
    },
    {
      name: 'slug_1',
      columns: ['slug'],
      isUnique: true,
      isPrimary: false,
      indexType: 'btree',
    },
    {
      name: 'categorias_1',
      columns: ['categorias'],
      isUnique: false,
      isPrimary: false,
      indexType: 'multikey',
    },
  ],
};

const avaliacoes: MockTable = {
  name: 'avaliacoes',
  schema: '',
  tableType: 'table',
  rowEstimate: 15_000,
  columns: [
    objectIdField(),
    field('produtoId', 'objectId', gen.objectId(), { isNullable: false }),
    field('autor', 'string', gen.fullName()),
    field('nota', 'int', gen.int(1, 5), { isNullable: false }),
    field('comentario', 'string', gen.sentence(5, 30, { nullRate: 0.25 })),
    field('util', 'int', gen.int(0, 480)),
    field('criadoEm', 'date', gen.timestamp(2024, 2026)),
  ],
  indexes: [
    {
      name: '_id_',
      columns: ['_id'],
      isUnique: true,
      isPrimary: true,
      indexType: 'btree',
    },
    {
      name: 'produtoId_1_nota_-1',
      columns: ['produtoId', 'nota'],
      isUnique: false,
      isPrimary: false,
      indexType: 'btree',
    },
  ],
};

const carrinhos: MockTable = {
  name: 'carrinhos',
  schema: '',
  tableType: 'table',
  rowEstimate: 640,
  columns: [
    objectIdField(),
    field('sessao', 'string', gen.uuid(), { isNullable: false }),
    field('itens', 'array', gen.intArray(1, 2_400, 1, 6)),
    field('total', 'double', gen.decimal(0, 8_000)),
    field('abandonado', 'bool', gen.bool()),
    field('atualizadoEm', 'date', gen.timestamp(2025, 2026)),
  ],
  indexes: [
    {
      name: '_id_',
      columns: ['_id'],
      isUnique: true,
      isPrimary: true,
      indexType: 'btree',
    },
  ],
};

const eventos: MockTable = {
  name: 'eventos',
  schema: '',
  tableType: 'table',
  rowEstimate: 30_000,
  columns: [
    objectIdField(),
    field('nivel', 'string', gen.pick(['debug', 'info', 'warn', 'error'])),
    field('mensagem', 'string', gen.sentence(4, 18)),
    field('contexto', 'object', gen.json({ nullRate: 0.3 })),
    field('ocorridoEm', 'date', gen.timestamp(2025, 2026)),
  ],
  indexes: [
    {
      name: '_id_',
      columns: ['_id'],
      isUnique: true,
      isPrimary: true,
      indexType: 'btree',
    },
    {
      name: 'ocorridoEm_-1',
      columns: ['ocorridoEm'],
      isUnique: false,
      isPrimary: false,
      indexType: 'btree',
    },
  ],
};

export function buildMongoDatabases(): MockDatabase[] {
  return [
    {
      name: 'catalogo',
      sizeBytes: 96_468_992,
      tables: [produtos, avaliacoes, carrinhos].map(clone),
    },
    { name: 'logs', sizeBytes: 2_147_483_648, tables: [eventos].map(clone) },
    { name: 'admin', sizeBytes: 40_960, tables: [] },
  ];
}

function clone(table: MockTable): MockTable {
  return { ...table, rows: undefined, nextSerial: undefined };
}
