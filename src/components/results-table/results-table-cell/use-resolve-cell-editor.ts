import type { CellEditorType } from './results-table-cell.types';

const TYPE_MAP: Record<string, CellEditorType> = {
  // Booleans
  bool: 'boolean',
  boolean: 'boolean',

  // Integers
  int2: 'number',
  int4: 'number',
  int8: 'number',
  smallint: 'number',
  integer: 'number',
  bigint: 'number',
  serial: 'number',
  bigserial: 'number',
  smallserial: 'number',

  // Floats
  float4: 'number',
  float8: 'number',
  real: 'number',
  'double precision': 'number',
  numeric: 'number',
  decimal: 'number',
  money: 'number',

  // Text
  text: 'text',
  varchar: 'text',
  'character varying': 'text',
  char: 'text',
  character: 'text',
  bpchar: 'text',
  name: 'text',
  citext: 'text',

  // JSON
  json: 'json',
  jsonb: 'json',

  // Date / Time — editados como texto normal para facilitar a digitação
  // (o backend recebe a string crua e faz o cast).
  date: 'text',
  time: 'text',
  'time without time zone': 'text',
  'time with time zone': 'text',
  timetz: 'text',
  timestamp: 'text',
  'timestamp without time zone': 'text',
  'timestamp with time zone': 'text',
  timestamptz: 'text',

  // UUID
  uuid: 'uuid',
};

/** O Postgres devolve booleano cru como "t"/"f"; outros adaptadores, "true"/"1". */
export function isBooleanTrue(value: string | null): boolean {
  return value === 'true' || value === 't' || value === '1';
}

/** Ciclo do checkbox booleano: NULL → true → false → true… */
export function nextBooleanValue(value: string | null): string {
  return value === null || !isBooleanTrue(value) ? 'true' : 'false';
}

export function resolveCellEditor(typeName: string): CellEditorType {
  const normalized = typeName.toLowerCase().trim();

  if (TYPE_MAP[normalized]) return TYPE_MAP[normalized];

  if (normalized.startsWith('_') || normalized.endsWith('[]')) {
    return 'json';
  }

  return 'text';
}
