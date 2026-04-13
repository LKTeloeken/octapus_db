import type { ExecuteQueryOptions } from '@/api/database/database-methods.types';
import type { ExecuteQueryResponse } from '@/api/database/database-responses.types';

export interface Tab {
  id: string;
  serverId: number;
  databaseName: string;
  title: string;
  content: string;
  type: TabType;
  viewSchema?: string;
  viewTable?: string;
  sort?: TabSort | null;
  loading: boolean;
  loadingMore: boolean;
  loadingChanges: boolean;
  runningQueryId?: string | null;
  queryOptions: ExecuteQueryOptions;
  lastExecutedQuery?: string;
  result?: ExecuteQueryResponse;
}

export interface TabSort {
  column: string;
  direction: 'asc' | 'desc';
}

export enum TabType {
  View = 'view',
  Query = 'query',
}
