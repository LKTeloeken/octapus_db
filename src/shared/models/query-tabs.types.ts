import type { ExecuteQueryOptions } from '@/api/database/database-methods.types';
import type { ExecuteQueryResponse } from '@/api/database/database-responses.types';

export interface QueryTab {
  serverId: number;
  databaseName: string;
  loading: boolean;
  loadingMore: boolean;
  queryOptions: ExecuteQueryOptions;
  lastExecutedQuery?: string;
  result?: ExecuteQueryResponse;
}
