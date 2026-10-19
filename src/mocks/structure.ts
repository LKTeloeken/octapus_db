import type { DatabaseType } from '@/api/types/server.types';
import type {
  ColumnDefinition,
  StructureChange,
} from '@/api/types/structure.types';
import { mockCatalog } from './catalog';
import {
  qualify,
  requireDatabase,
  requireServerEntry,
  type MockColumn,
  type MockTable,
} from './data';

/**
 * Edição de estrutura (DDL) simulada: o SQL da prévia imita o que o backend
 * monta (`adapters/postgres/ddl.rs` e `adapters/sqlite/ddl.rs`) e a aplicação
 * mexe nas tabelas do dataset em memória. O `saas` é gerado: não se edita.
 */

const quote = (name: string) => `"${name.trim().replace(/"/g, '""')}"`;

function columnSql(column: ColumnDefinition, inlinePk: boolean): string {
  let sql = `${quote(column.name)} ${column.dataType.trim()}`;
  if (!column.nullable) sql += ' NOT NULL';
  if (column.defaultValue?.trim())
    sql += ` DEFAULT ${column.defaultValue.trim()}`;
  if (inlinePk && column.primaryKey) sql += ' PRIMARY KEY';
  return sql;
}

const sameType = (a: string, b: string) =>
  a.trim().replace(/\s+/g, ' ').toLowerCase() ===
  b.trim().replace(/\s+/g, ' ').toLowerCase();

const sameDefault = (a: string | null, b: string | null) =>
  (a?.trim() || null) === (b?.trim() || null);

/** Os mesmos erros de validação do backend (`adapters/ddl.rs`), em resumo */
function validate(change: StructureChange): void {
  const columns =
    change.type === 'createTable'
      ? change.columns
      : change.type === 'addColumn'
        ? [change.column]
        : change.type === 'alterColumn'
          ? [change.updated]
          : [];
  if (!change.table.trim()) throw 'Invalid query: Table name is required';
  if (change.type === 'createTable' && columns.length === 0) {
    throw 'Invalid query: A table needs at least one column';
  }
  for (const column of columns) {
    if (!column.name.trim()) throw 'Invalid query: Column name is required';
    if (!column.dataType.trim()) throw 'Invalid query: Column type is required';
    if (/[;'\-/*]/.test(column.dataType)) {
      throw `Invalid query: Invalid column type: ${column.dataType.trim()}`;
    }
    if (column.defaultValue?.includes(';')) {
      throw 'Invalid query: Default value must be a single expression';
    }
  }
}

export function structureSql(
  dbType: DatabaseType,
  change: StructureChange,
): string[] {
  validate(change);
  const sqlite = dbType === 'sqlite';
  const target =
    change.schema && !sqlite
      ? `${quote(change.schema)}.${quote(change.table)}`
      : quote(change.table);

  switch (change.type) {
    case 'createTable': {
      const keys = change.columns.filter(column => column.primaryKey);
      const body = change.columns.map(
        column => `  ${columnSql(column, false)}`,
      );
      if (keys.length) {
        body.push(
          `  PRIMARY KEY (${keys.map(key => quote(key.name)).join(', ')})`,
        );
      }
      return [`CREATE TABLE ${target} (\n${body.join(',\n')}\n)`];
    }
    case 'renameTable':
      return [`ALTER TABLE ${target} RENAME TO ${quote(change.newName)}`];
    case 'dropTable': {
      const what = {
        table: 'TABLE',
        view: 'VIEW',
        materializedView: sqlite ? 'TABLE' : 'MATERIALIZED VIEW',
        foreign: sqlite ? 'TABLE' : 'FOREIGN TABLE',
      }[change.kind];
      return [
        `DROP ${what} ${target}${change.cascade && !sqlite ? ' CASCADE' : ''}`,
      ];
    }
    case 'addColumn':
      return [
        `ALTER TABLE ${target} ADD COLUMN ${columnSql(change.column, true)}`,
      ];
    case 'dropColumn':
      return [
        `ALTER TABLE ${target} DROP COLUMN ${quote(change.column)}${change.cascade && !sqlite ? ' CASCADE' : ''}`,
      ];
    case 'alterColumn': {
      const { original, updated } = change;
      const column = quote(updated.name);
      const statements: string[] = [];
      if (original.name.trim() !== updated.name.trim()) {
        statements.push(
          `ALTER TABLE ${target} RENAME COLUMN ${quote(original.name)} TO ${column}`,
        );
      }
      const typeChanged = !sameType(original.dataType, updated.dataType);
      const defaultChanged = !sameDefault(
        original.defaultValue,
        updated.defaultValue,
      );
      const nullChanged = original.nullable !== updated.nullable;

      if (sqlite) {
        if (typeChanged || defaultChanged || nullChanged) {
          const temp = quote(`_octapus_new_${change.table}`);
          statements.push(
            'PRAGMA legacy_alter_table = ON',
            `CREATE TABLE ${temp} (… ${columnSql(updated, false)} …)`,
            `INSERT INTO ${temp} SELECT * FROM ${target}`,
            `DROP TABLE ${target}`,
            `ALTER TABLE ${temp} RENAME TO ${target}`,
          );
        }
        return statements;
      }

      const dataType = updated.dataType.trim();
      if (typeChanged) {
        statements.push(
          `ALTER TABLE ${target} ALTER COLUMN ${column} TYPE ${dataType} USING ${column}::${dataType}`,
        );
      }
      if (defaultChanged) {
        statements.push(
          updated.defaultValue?.trim()
            ? `ALTER TABLE ${target} ALTER COLUMN ${column} SET DEFAULT ${updated.defaultValue.trim()}`
            : `ALTER TABLE ${target} ALTER COLUMN ${column} DROP DEFAULT`,
        );
      }
      if (nullChanged) {
        statements.push(
          `ALTER TABLE ${target} ALTER COLUMN ${column} ${updated.nullable ? 'DROP' : 'SET'} NOT NULL`,
        );
      }
      return statements;
    }
  }
}

const toMockColumn = (column: ColumnDefinition): MockColumn => ({
  name: column.name.trim(),
  dataType: column.dataType.trim(),
  typeOid: null,
  isNullable: column.nullable,
  defaultValue: column.defaultValue?.trim() || null,
  isPrimaryKey: column.primaryKey,
  isForeignKey: false,
  // Coluna nova começa vazia nas linhas que já existem
  gen: () => null,
});

export function applyStructure(
  serverId: number,
  database: string,
  change: StructureChange,
): void {
  const entry = requireServerEntry(serverId);
  structureSql(entry.server.dbType, change);
  const db = requireDatabase(entry, database);
  if (db.generated) {
    throw 'Unsupported type: o mock não edita a estrutura do database gerado';
  }

  const schema = entry.capabilities.hasSchemas
    ? (change.schema ?? 'public')
    : '';
  const find = (name: string) =>
    db.tables.find(table => table.name === name && table.schema === schema);
  const require = (name: string): MockTable => {
    const found = find(name);
    if (!found)
      throw `Query error: relation "${qualify(schema, name)}" does not exist`;
    return found;
  };
  const columnIndex = (table: MockTable, name: string) => {
    const index = table.columns.findIndex(column => column.name === name);
    if (index < 0) throw `Query error: column "${name}" does not exist`;
    return index;
  };

  switch (change.type) {
    case 'createTable':
      if (find(change.table.trim())) {
        throw `Query error: relation "${change.table.trim()}" already exists`;
      }
      db.tables.push({
        name: change.table.trim(),
        schema,
        tableType: 'table',
        columns: change.columns.map(toMockColumn),
        indexes: [],
        rowEstimate: 0,
        rows: [],
      });
      break;
    case 'renameTable': {
      const table = require(change.table);
      if (find(change.newName.trim())) {
        throw `Query error: relation "${change.newName.trim()}" already exists`;
      }
      table.name = change.newName.trim();
      break;
    }
    case 'dropTable':
      db.tables.splice(db.tables.indexOf(require(change.table)), 1);
      break;
    case 'addColumn': {
      const table = require(change.table);
      table.columns = [...table.columns, toMockColumn(change.column)];
      table.rows?.forEach(row => row.push(null));
      break;
    }
    case 'dropColumn': {
      const table = require(change.table);
      const index = columnIndex(table, change.column);
      table.columns = table.columns.filter((_, i) => i !== index);
      table.rows?.forEach(row => row.splice(index, 1));
      break;
    }
    case 'alterColumn': {
      const table = require(change.table);
      const index = columnIndex(table, change.original.name);
      if (
        !change.updated.nullable &&
        table.rows?.some(row => row[index] === null)
      ) {
        throw `Query error: column "${change.updated.name.trim()}" of relation "${table.name}" contains null values`;
      }
      const current = table.columns[index];
      table.columns = table.columns.map((column, i) =>
        i === index
          ? {
              ...current,
              name: change.updated.name.trim(),
              dataType: change.updated.dataType.trim(),
              typeName: undefined,
              isNullable: change.updated.nullable,
              defaultValue: change.updated.defaultValue?.trim() || null,
            }
          : column,
      );
      break;
    }
  }

  mockCatalog.structureChanged(serverId, database, db);
}
