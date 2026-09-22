import type { CellEditorType } from '@/components/results-table/results-table-cell/results-table-cell.types';
import {
  isBooleanTrue,
  resolveCellEditor,
} from '@/components/results-table/results-table-cell/use-resolve-cell-editor';
import type { DataTableRow } from '../results-table/results-table.types';
import type {
  ExportColumn,
  ExportFormat,
  ExportOptions,
} from './export-dialog.types';

export const EXTENSIONS: Record<ExportFormat, string> = {
  csv: 'csv',
  json: 'json',
  sql: 'sql',
};

/** Literal que pode ir cru no INSERT sem virar string */
const NUMERIC_LITERAL = /^-?\d+(\.\d+)?([eE][+-]?\d+)?$/;

export function serializeRows(
  columns: ExportColumn[],
  rows: DataTableRow[],
  options: ExportOptions,
): string {
  switch (options.format) {
    case 'csv':
      return toCsv(columns, rows, options);
    case 'json':
      return toJson(columns, rows);
    case 'sql':
      return toSql(columns, rows, options.sqlTable);
  }
}

// ── CSV ─────────────────────────────────────────────────────────────────────

/**
 * RFC 4180: aspas dobram dentro do campo e só se aspeia o que precisa. NULL sai
 * como campo vazio e a string vazia como `""`, para os dois não se confundirem.
 */
function toCsv(
  columns: ExportColumn[],
  rows: DataTableRow[],
  { delimiter, includeHeader }: ExportOptions,
): string {
  const field = (value: string | null) => {
    if (value === null) return '';
    if (value === '' || value.includes(delimiter) || /["\n\r]/.test(value)) {
      return `"${value.replace(/"/g, '""')}"`;
    }
    return value;
  };

  const lines: string[] = [];
  if (includeHeader) {
    lines.push(columns.map(column => field(column.name)).join(delimiter));
  }
  for (const row of rows) {
    lines.push(
      columns.map(column => field(row[column.index] ?? null)).join(delimiter),
    );
  }

  return lines.join('\n');
}

// ── JSON ────────────────────────────────────────────────────────────────────

function toJson(columns: ExportColumn[], rows: DataTableRow[]): string {
  const objects = rows.map(row =>
    Object.fromEntries(
      columns.map(column => [column.name, row[column.index] ?? null]),
    ),
  );

  return JSON.stringify(objects, null, 2);
}

// ── SQL ─────────────────────────────────────────────────────────────────────

function toSql(
  columns: ExportColumn[],
  rows: DataTableRow[],
  sqlTable: string,
): string {
  const columnList = columns.map(column => quoteIdent(column.name)).join(', ');
  const editors = columns.map(column => resolveCellEditor(column.typeName));

  return rows
    .map(row => {
      const values = columns
        .map((column, index) =>
          literal(row[column.index] ?? null, editors[index]),
        )
        .join(', ');
      return `INSERT INTO ${sqlTable} (${columnList}) VALUES (${values});`;
    })
    .join('\n');
}

/** O Postgres devolve booleano como "t"/"f", que nenhum INSERT aceita de volta */
function literal(value: string | null, editor: CellEditorType): string {
  if (value === null) return 'NULL';
  if (editor === 'boolean') return isBooleanTrue(value) ? 'true' : 'false';
  if (editor === 'number' && NUMERIC_LITERAL.test(value)) return value;
  return `'${value.replace(/'/g, "''")}'`;
}

export function quoteIdent(name: string): string {
  return `"${name.replace(/"/g, '""')}"`;
}
