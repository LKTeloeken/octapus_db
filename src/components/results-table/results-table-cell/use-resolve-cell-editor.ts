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

  // Date / Time
  date: 'date',
  time: 'time',
  'time without time zone': 'time',
  'time with time zone': 'time',
  timetz: 'time',
  timestamp: 'datetime',
  'timestamp without time zone': 'datetime',
  'timestamp with time zone': 'datetime',
  timestamptz: 'datetime',

  // UUID
  uuid: 'uuid',
};

export function resolveCellEditor(typeName: string): CellEditorType {
  const normalized = typeName.toLowerCase().trim();

  if (TYPE_MAP[normalized]) return TYPE_MAP[normalized];

  if (normalized.startsWith('_') || normalized.endsWith('[]')) {
    return 'json';
  }

  return 'text';
}
