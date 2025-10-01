export type ColumnKey = string;

export interface QueryResultsColumn {
  /** Key in the row object to read the value from */
  key: ColumnKey;
  /** Optional label to display in the header; defaults to key */
  label?: string;
  /** Optional width (px, %, fr). Applied as a style on header/cells */
  width?: number | string;
  /** Text alignment for cells in this column */
  align?: "left" | "center" | "right";
}

export type QueryResultsRow = Record<string, unknown>;

export interface QueryResultsTableProps {
  /** Column definitions or simple keys (string) */
  columns: Array<string>;
  /** Array of row objects. Values can be string/number/ReactNode */
  rows: QueryResultsRow[];
  /** Fixed row height in pixels used for virtualization math. Default: 36 */
  rowHeight?: number;
  /** Extra rows to render above/below the viewport. Default: 8 */
  overscan?: number;
  /** Optional className applied to the outer container */
  className?: string;
  /** Message when there are no rows */
  emptyMessage?: string;
}
