import type { CellEditorType } from './results-table-cell.types';

/**
 * Cor do valor por tipo de dado (DESIGN.md §2.6): croma baixo, tinge sem
 * gritar. NULL não entra aqui — é sempre `--fg-subtle` em itálico.
 */
export const CELL_TYPE_COLOR: Record<CellEditorType, string> = {
  text: 'text-fg',
  number: 'text-data-number',
  boolean: 'text-data-bool',
  json: 'text-data-json',
  array: 'text-data-json',
  date: 'text-data-date',
  datetime: 'text-data-date',
  time: 'text-data-date',
  uuid: 'text-fg-muted',
  enum: 'text-fg',
  readonly: 'text-fg-muted',
};
