import type { MockColumn, MockDatabase, MockTable } from './types';
import * as gen from './rows';

/**
 * O SQLite é um arquivo só: um único banco (`main`), sem nível de schema e com
 * os tipos declarados do `CREATE TABLE` (INTEGER/TEXT/REAL/BOOLEAN), que é o
 * que o adapter devolve em `typeName`. Tem PK, então o grid abre editável.
 */
type ColumnOverrides = Partial<Omit<MockColumn, 'name' | 'gen'>>;

const column = (
  name: string,
  dataType: string,
  cell: gen.CellGen,
  overrides: ColumnOverrides = {},
): MockColumn => ({
  name,
  dataType,
  // Sem equivalente ao OID do Postgres
  typeOid: null,
  isNullable: true,
  defaultValue: null,
  isPrimaryKey: false,
  isForeignKey: false,
  gen: cell,
  ...overrides,
});

const pk = (): MockColumn =>
  column('id', 'INTEGER', gen.serial(), {
    isNullable: false,
    isPrimaryKey: true,
  });

/** No SQLite booleano é 1/0 — não existe tipo lógico próprio. */
const flag = (): gen.CellGen => gen.pick(['1', '0']);

const USUARIOS = 1_200;
const NOTAS = 4_800;

const usuarios: MockTable = {
  name: 'usuarios',
  // Adapter sem schema, igual ao Mongo e ao Redis
  schema: '',
  tableType: 'table',
  rowEstimate: USUARIOS,
  columns: [
    pk(),
    column('nome', 'TEXT', gen.fullName(), { isNullable: false }),
    column('email', 'TEXT', gen.email(), { isNullable: false }),
    column('ativo', 'BOOLEAN', flag(), {
      isNullable: false,
      defaultValue: '1',
    }),
    column('saldo', 'REAL', gen.decimal(0, 5_000)),
    column('criado_em', 'TEXT', gen.timestamp()),
  ],
  indexes: [
    {
      name: 'idx_usuarios_email',
      columns: ['email'],
      isUnique: true,
      isPrimary: false,
      indexType: 'btree',
    },
  ],
};

const notas: MockTable = {
  name: 'notas',
  schema: '',
  tableType: 'table',
  rowEstimate: NOTAS,
  columns: [
    pk(),
    column('usuario_id', 'INTEGER', gen.fk(USUARIOS), {
      isNullable: false,
      isForeignKey: true,
    }),
    column('titulo', 'TEXT', gen.sentence(2, 6), { isNullable: false }),
    column('conteudo', 'TEXT', gen.sentence(8, 40, { nullRate: 0.15 })),
    column('fixada', 'BOOLEAN', flag(), { isNullable: false, defaultValue: '0' }),
    column('atualizada_em', 'TEXT', gen.timestamp()),
  ],
  indexes: [
    {
      name: 'idx_notas_usuario',
      columns: ['usuario_id'],
      isUnique: false,
      isPrimary: false,
      indexType: 'btree',
    },
  ],
};

const tags: MockTable = {
  name: 'tags',
  schema: '',
  tableType: 'table',
  rowEstimate: 42,
  columns: [
    pk(),
    column('nome', 'TEXT', gen.pick(['pessoal', 'trabalho', 'ideias', 'urgente']), {
      isNullable: false,
    }),
    column('cor', 'TEXT', gen.pick(['#ef4444', '#22c55e', '#3b82f6', '#eab308'])),
  ],
  indexes: [],
};

export function buildSqliteDatabases(): MockDatabase[] {
  return [
    {
      name: 'main',
      sizeBytes: 3_284_992,
      tables: [notas, tags, usuarios],
    },
  ];
}
