import type { DatabaseType } from '@/api/types/server.types';

/** A single cached table, flattened from the structure cache. */
export interface TableEntry {
  /** encodeNodeId of the table — also the browse tab id */
  id: string;
  serverId: number;
  serverName: string;
  dbType: DatabaseType;
  database: string;
  /** null for schema-less databases (Mongo/Redis) */
  schema: string | null;
  table: string;
  /** Fuzzy target & display: `schema/table` or `table` */
  label: string;
}

export interface PaletteItem {
  entry: TableEntry;
  /** Indices in `entry.label` to highlight */
  indices: number[];
  /** Secondary line below the label (already resolved for the group context) */
  subtitle: string;
}

export interface ResultGroup {
  key: string;
  heading: string;
  items: PaletteItem[];
}

/**
 * Flattened row model for the virtualized list — group headings and items are
 * interleaved into a single array so one virtualizer can scroll the whole list.
 */
export type PaletteRow =
  | { kind: 'header'; key: string; heading: string }
  | { kind: 'item'; key: string; item: PaletteItem };
