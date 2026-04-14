import type { DatabaseStructure, Column } from '@/shared/models/database.types';

export interface SchemaCacheState {
  // Key format: "serverId:databaseName"
  cache: Record<string, CacheEntry>;
  loading: Record<string, boolean>;
  recentOpenedTables: RecentOpenedTable[];
  viewLayout: ResultsViewLayout;

  // Actions
  fetchStructure: FetchStructure;
  fetchColumns: FetchColumns;
  getStructure: GetStructure;
  invalidate: Invalidate;
  clearAll: ClearAll;
  recordRecentOpenedTable: RecordRecentOpenedTable;
  setViewLayout: SetViewLayout;
}

export type FetchStructure = (
  serverId: number,
  databaseName: string,
  forceRefresh?: boolean,
) => Promise<DatabaseStructure>;

export type GetStructure = (
  serverId: number,
  databaseName: string,
) => DatabaseStructure | null;

export type FetchColumns = (
  serverId: number,
  databaseName: string,
  schemaName: string,
  tableName: string,
) => Promise<Column[]>;

export type Invalidate = (serverId: number, databaseName?: string) => void;

export type ClearAll = () => void;

export type ResultsViewLayout = 'horizontal' | 'vertical';

export interface RecentOpenedTable {
  key: string;
  serverId: number;
  serverName: string;
  databaseName: string;
  schemaName: string;
  tableName: string;
  openedAt: number;
}

export type RecordRecentOpenedTable = (
  item: Omit<RecentOpenedTable, 'openedAt'>,
) => void;

export type SetViewLayout = (layout: ResultsViewLayout) => void;

export interface CacheEntry {
  structure: DatabaseStructure;
  fetchedAt: number;
}
