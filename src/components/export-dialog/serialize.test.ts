import { describe, expect, it } from 'vitest';
import { serializeRows } from './serialize';

const columns = [
  { name: 'id', typeName: 'int4', index: 0 },
  { name: 'name', typeName: 'text', index: 1 },
  { name: 'created_at', typeName: 'timestamptz', index: 2 },
];
const rows = [['1', "O'Neil", '2026-01-01 00:00:00+00']];
const options = {
  format: 'sql' as const,
  delimiter: ',',
  includeHeader: true,
  sqlTable: '"public"."users"',
};

describe('serializeRows — SQL', () => {
  it('sem a opção, todas as colunas entram', () => {
    expect(serializeRows(columns, rows, options)).toBe(
      `INSERT INTO "public"."users" ("id", "name", "created_at") VALUES (1, 'O''Neil', '2026-01-01 00:00:00+00');`,
    );
  });

  it('colunas com valor padrão ficam fora para o banco preencher', () => {
    expect(
      serializeRows(columns, rows, {
        ...options,
        sqlDefaultColumns: new Set(['id', 'created_at']),
      }),
    ).toBe(`INSERT INTO "public"."users" ("name") VALUES ('O''Neil');`);
  });

  it('todas com valor padrão: DEFAULT VALUES', () => {
    expect(
      serializeRows(columns.slice(0, 1), rows, {
        ...options,
        sqlDefaultColumns: new Set(['id']),
      }),
    ).toBe('INSERT INTO "public"."users" DEFAULT VALUES;');
  });
});
