import type { ExecuteQueryOptions } from '@/api/database/database-methods.types';
import type { ExecuteQueryResponse } from '@/api/database/database-responses.types';

export interface Tab {
  id: string;
  serverId: number;
  databaseName: string;
  title: string;
  content: string;
  type: TabType;
  loading: boolean;
  loadingMore: boolean;
  loadingChanges: boolean;
  queryOptions: ExecuteQueryOptions;
  viewOrder?: 'asc' | 'desc';
  lastExecutedQuery?: string;
  result?: ExecuteQueryResponse;
}

export enum TabType {
  View = 'view',
  Query = 'query',
}
