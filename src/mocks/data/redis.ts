import type { MockColumn, MockDatabase, MockTable } from './types';
import * as gen from './rows';

/**
 * O adapter do Redis expõe uma forma tabular fixa: cada "tabela" é um prefixo
 * de chave e o browse devolve sempre key/type/value/ttl. Sem PK
 * (`hasPrimaryKeys: false`), então o grid abre em modo leitura.
 */
const REDIS_TYPES = ['string', 'hash', 'list', 'set', 'zset'] as const;

const cell = (
  name: string,
  typeName: string,
  value: gen.CellGen,
  isNullable = true,
): MockColumn => ({
  name,
  dataType: typeName,
  typeOid: null,
  isNullable,
  defaultValue: null,
  isPrimaryKey: false,
  isForeignKey: false,
  gen: value,
});

const keyGroup = (prefix: string, rowEstimate: number): MockTable => ({
  name: prefix,
  schema: '',
  tableType: 'table',
  rowEstimate,
  columns: [
    cell(
      'key',
      'string',
      (rng, row) => `${prefix}${redisSuffix(rng, prefix, row)}`,
      false,
    ),
    cell('type', 'string', gen.pick(REDIS_TYPES), false),
    cell('value', 'string', gen.sentence(2, 10)),
    // TTL -1 é "sem expiração", igual ao retorno do próprio Redis.
    cell('ttl', 'long', rng =>
      rng() < 0.4 ? '-1' : String(60 + Math.floor(rng() * 86_400)),
    ),
  ],
  indexes: [],
});

function redisSuffix(rng: gen.Rng, prefix: string, row: number) {
  if (prefix === 'session:') return gen.uuid()(rng, row);
  return String(row + 1);
}

/** Redis numera os bancos de 0 a 15; só alguns têm chaves no mock. */
export function buildRedisDatabases(): MockDatabase[] {
  const populated: Record<string, MockTable[]> = {
    '0': [
      keyGroup('user:', 4_200),
      keyGroup('session:', 1_800),
      keyGroup('cart:', 640),
    ],
    '1': [keyGroup('feature:', 48), keyGroup('ratelimit:', 2_600)],
    '3': [keyGroup('job:', 320)],
  };

  return Array.from({ length: 16 }, (_, index) => {
    const name = String(index);
    const tables = populated[name] ?? [];
    return {
      name,
      sizeBytes: tables.length > 0 ? tables.length * 1_048_576 : 0,
      tables: tables.map(clone),
    };
  });
}

function clone(table: MockTable): MockTable {
  return { ...table, rows: undefined, nextSerial: undefined };
}
