import type { DatabaseType } from '@/api/types/server.types';
import type { ColumnDefinition } from '@/api/types/structure.types';

/** Sugestões do campo de tipo — o campo aceita qualquer tipo do banco */
export const TYPE_SUGGESTIONS: Partial<Record<DatabaseType, string[]>> = {
  postgres: [
    'bigint',
    'integer',
    'smallint',
    'bigserial',
    'serial',
    'numeric(10, 2)',
    'real',
    'double precision',
    'boolean',
    'text',
    'varchar(255)',
    'uuid',
    'date',
    'timestamp',
    'timestamptz',
    'jsonb',
    'json',
    'bytea',
    'text[]',
  ],
  sqlite: ['INTEGER', 'TEXT', 'REAL', 'NUMERIC', 'BLOB', 'BOOLEAN', 'DATETIME'],
};

/** A primeira coluna de uma tabela nova: a chave */
export function defaultKeyColumn(dbType: DatabaseType): ColumnDefinition {
  return {
    name: 'id',
    dataType: dbType === 'sqlite' ? 'INTEGER' : 'bigserial',
    nullable: false,
    defaultValue: null,
    primaryKey: true,
  };
}

export function emptyColumn(): ColumnDefinition {
  return {
    name: '',
    dataType: '',
    nullable: true,
    defaultValue: null,
    primaryKey: false,
  };
}

/** Id do `<datalist>` de tipos de um banco */
export const typeListId = (dbType: DatabaseType) => `structure-types-${dbType}`;
