import type { SortSpec } from '@/api/types/browse.types';
import type {
  EditableInfo,
  QueryResult,
  RowEdit,
  RowInsert,
} from '@/api/types/query.types';
import {
  primaryKeyOf,
  takeSerial,
  tableRows,
  toQueryColumnInfo,
  type MockColumn,
  type MockTable,
} from './data';

type Row = (string | null)[];

const NUMERIC_TYPES =
  /^(smallint|integer|bigint|numeric|decimal|real|double precision|int|int2|int4|int8|float|float4|float8|long|serial|bigserial)$/i;

const isNumericColumn = (column: MockColumn) =>
  NUMERIC_TYPES.test(column.dataType.trim());

export function columnIndex(table: MockTable, name: string): number {
  const index = table.columns.findIndex(column => column.name === name);
  if (index < 0) throw `Query error: column "${name}" does not exist`;
  return index;
}

// ── Ordenação ───────────────────────────────────────────────────────────────

/** Nulos sempre por último, como `NULLS LAST` do Postgres */
function compare(a: string | null, b: string | null, numeric: boolean): number {
  if (a === b) return 0;
  if (a === null) return 1;
  if (b === null) return -1;
  if (numeric) {
    const left = Number(a);
    const right = Number(b);
    if (!Number.isNaN(left) && !Number.isNaN(right)) return left - right;
  }
  return a.localeCompare(b, 'pt-BR', { numeric: true });
}

export function applySort(
  rows: Row[],
  table: MockTable,
  sort: SortSpec[] | undefined,
): Row[] {
  if (!sort || sort.length === 0) return rows;

  const specs = sort.map(spec => {
    const index = columnIndex(table, spec.column);
    return {
      index,
      numeric: isNumericColumn(table.columns[index]),
      sign: spec.direction === 'desc' ? -1 : 1,
    };
  });

  // Cópia: a ordem do array de origem é o estado persistido da tabela.
  return [...rows].sort((left, right) => {
    for (const spec of specs) {
      const result = compare(left[spec.index], right[spec.index], spec.numeric);
      if (result !== 0) return result * spec.sign;
    }
    return 0;
  });
}

// ── Filtro (`whereExpr`) ────────────────────────────────────────────────────

type Predicate = (row: Row) => boolean;

const CONDITION =
  /^\s*("?[\w.]+"?)\s+(=|<>|!=|>=|<=|>|<|(?:not\s+)?i?like|in|is(?:\s+not)?\s+null)\s*([\s\S]*)$/i;

/**
 * Parser mínimo, só o suficiente para o filtro do table-browser:
 * `col = 'x'`, `col <> 1`, `col LIKE '%a%'`, `col IN ('a','b')`, `col IS NULL`,
 * ligados por AND/OR (sem parênteses). Qualquer outra coisa rejeita com a
 * mesma cara de um erro de sintaxe do Postgres — é assim que se testa o estado
 * de erro do campo.
 */
export function applyWhere(
  rows: Row[],
  table: MockTable,
  whereExpr: string | undefined | null,
): Row[] {
  const expression = whereExpr?.trim();
  if (!expression) return rows;

  if (/[()]/.test(expression) && !/\bin\b/i.test(expression)) {
    throw `Query error: syntax error at or near "("`;
  }

  const parts = expression.split(/\s+(AND|OR)\s+/i);
  const first = buildPredicate(table, parts[0]);

  let predicate = first;
  for (let i = 1; i < parts.length; i += 2) {
    const operator = parts[i].toUpperCase();
    const next = buildPredicate(table, parts[i + 1] ?? '');
    const left = predicate;
    predicate =
      operator === 'OR'
        ? row => left(row) || next(row)
        : row => left(row) && next(row);
  }

  return rows.filter(predicate);
}

function buildPredicate(table: MockTable, clause: string): Predicate {
  const match = CONDITION.exec(clause);
  if (!match)
    throw `Query error: syntax error at or near "${clause.trim().split(/\s+/)[0] ?? ''}"`;

  const [, rawColumn, rawOperator, rawValue] = match;
  const index = columnIndex(table, rawColumn.replace(/"/g, ''));
  const numeric = isNumericColumn(table.columns[index]);
  const operator = rawOperator.replace(/\s+/g, ' ').toLowerCase();

  if (operator.startsWith('is')) {
    const negated = operator.includes('not');
    return row => (row[index] === null) !== negated;
  }

  if (operator === 'in') {
    const items = parseList(rawValue);
    return row => row[index] !== null && items.includes(row[index] as string);
  }

  if (operator.endsWith('like')) {
    const pattern = likeToRegExp(unquote(rawValue), operator.includes('ilike'));
    const negated = operator.startsWith('not');
    return row =>
      row[index] !== null && pattern.test(row[index] as string) !== negated;
  }

  const raw = rawValue.trim();
  const quoted = /^'[\s\S]*'$/.test(raw);

  // Sem aspas só passa número, NULL ou booleano — qualquer outra coisa é erro
  // de sintaxe, senão `total >> 5` viraria uma comparação silenciosa.
  if (!quoted && !/^(-?\d+(?:\.\d+)?|null|true|false)$/i.test(raw)) {
    throw `Query error: syntax error at or near "${raw.split(/\s+/)[0] || rawOperator}"`;
  }

  // `col = NULL` nunca é verdadeiro em SQL — só `IS NULL` casa.
  if (!quoted && /^null$/i.test(raw)) return () => false;

  const value = unquote(raw);
  return row => {
    const cell = row[index];
    if (cell === null) return false;
    const result = compare(cell, value, numeric);
    switch (operator) {
      case '=':
        return result === 0;
      case '<>':
      case '!=':
        return result !== 0;
      case '>':
        return result > 0;
      case '>=':
        return result >= 0;
      case '<':
        return result < 0;
      case '<=':
        return result <= 0;
      default:
        throw `Query error: operator does not exist: ${operator}`;
    }
  };
}

function unquote(raw: string): string {
  const value = raw.trim();
  if (/^'[\s\S]*'$/.test(value)) return value.slice(1, -1).replace(/''/g, "'");
  return value;
}

function parseList(raw: string): string[] {
  const inner = raw.trim().replace(/^\(/, '').replace(/\)$/, '');
  return inner.split(',').map(unquote);
}

function likeToRegExp(pattern: string, insensitive: boolean): RegExp {
  const escaped = pattern.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  // `[\s\S]` no lugar de `.` para casar quebra de linha sem depender da flag `s`.
  const source = escaped.replace(/%/g, '[\\s\\S]*').replace(/_/g, '[\\s\\S]');
  return new RegExp(`^${source}$`, insensitive ? 'i' : '');
}

// ── Paginação / montagem do QueryResult ─────────────────────────────────────

export interface PageOptions {
  limit?: number;
  offset?: number;
  countTotal?: boolean;
  unlimited?: boolean;
  /** Postgres-only, ignorado nos demais */
  whereExpr?: string | null;
  sort?: SortSpec[];
}

export function editableInfoFor(table: MockTable): EditableInfo | null {
  const primaryKeyColumns = primaryKeyOf(table);
  if (primaryKeyColumns.length === 0 || table.tableType !== 'table')
    return null;

  return {
    schema: table.schema,
    table: table.name,
    primaryKeyColumns,
    primaryKeyColumnIndices: primaryKeyColumns.map(name =>
      columnIndex(table, name),
    ),
  };
}

/** Monta um QueryResult com a mesma semântica do `fetch_table_data` do Rust */
export function selectFrom(
  table: MockTable,
  options: PageOptions,
  startedAt = performance.now(),
): QueryResult {
  const filtered = applyWhere(tableRows(table), table, options.whereExpr);
  const sorted = applySort(filtered, table, options.sort);

  const offset = Math.max(0, options.offset ?? 0);
  const limit = options.unlimited
    ? sorted.length
    : Math.max(0, options.limit ?? 500);
  const page = sorted.slice(offset, offset + limit);

  return {
    columns: table.columns.map(toQueryColumnInfo),
    rows: page,
    rowCount: page.length,
    totalCount: options.countTotal ? sorted.length : null,
    hasMore: offset + page.length < sorted.length,
    executionTimeMs: Math.round(performance.now() - startedAt),
    editableInfo: editableInfoFor(table),
  };
}

export function emptyResult(table?: MockTable): QueryResult {
  return {
    columns: table ? table.columns.map(toQueryColumnInfo) : [],
    rows: [],
    rowCount: 0,
    totalCount: 0,
    hasMore: false,
    executionTimeMs: 1,
    editableInfo: table ? editableInfoFor(table) : null,
  };
}

// ── Mutações ────────────────────────────────────────────────────────────────

function findRowIndex(
  table: MockTable,
  editable: EditableInfo,
  pkValues: (string | null)[],
): number {
  const indices = editable.primaryKeyColumns.map(name =>
    columnIndex(table, name),
  );
  return tableRows(table).findIndex(row =>
    indices.every((index, position) => row[index] === pkValues[position]),
  );
}

export function applyEdits(
  table: MockTable,
  editable: EditableInfo,
  edits: RowEdit[],
): number {
  const rows = tableRows(table);
  let affected = 0;

  for (const edit of edits) {
    const rowIndex = findRowIndex(table, editable, edit.pkValues);
    if (rowIndex < 0) {
      throw `Query error: row not found for primary key (${edit.pkValues.join(', ')})`;
    }
    const next = [...rows[rowIndex]];
    for (const [name, value] of edit.changes) {
      next[columnIndex(table, name)] = value;
    }
    rows[rowIndex] = next;
    affected++;
  }

  return affected;
}

export function applyInserts(
  table: MockTable,
  rowsToInsert: RowInsert[],
): number {
  const rows = tableRows(table);

  for (const insert of rowsToInsert) {
    const filled = new Map(insert.values);
    const row = table.columns.map(column => {
      if (filled.has(column.name)) return filled.get(column.name) ?? null;
      // Colunas omitidas caem no default do banco — aqui, no default declarado
      // ou no próximo valor da sequência quando é PK serial.
      if (
        column.isPrimaryKey &&
        /serial|nextval/i.test(column.defaultValue ?? '')
      ) {
        return String(takeSerial(table));
      }
      if (column.defaultValue === 'now()') {
        return new Date().toISOString().slice(0, 19).replace('T', ' ') + '-03';
      }
      return (
        column.defaultValue?.replace(/::.*$/, '').replace(/^'|'$/g, '') ?? null
      );
    });
    rows.push(row);
  }

  return rowsToInsert.length;
}

export function applyDeletes(
  table: MockTable,
  editable: EditableInfo,
  pkValues: (string | null)[][],
): number {
  const rows = tableRows(table);
  const indices = editable.primaryKeyColumns.map(name =>
    columnIndex(table, name),
  );

  const doomed = new Set<number>();
  for (const key of pkValues) {
    const rowIndex = rows.findIndex(
      (row, position) =>
        !doomed.has(position) &&
        indices.every((index, slot) => row[index] === key[slot]),
    );
    if (rowIndex >= 0) doomed.add(rowIndex);
  }

  // De trás para frente para não invalidar os índices seguintes.
  for (const index of Array.from(doomed).sort((a, b) => b - a))
    rows.splice(index, 1);

  return doomed.size;
}
