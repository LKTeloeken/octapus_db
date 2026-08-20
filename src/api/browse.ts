import { call } from './client';
import { RustCommand } from './commands';
import type { TableDataRequest } from './types/browse.types';
import type { QueryResult } from './types/query.types';

/**
 * Server-side browse: the backend builds the SQL/find/scan and paginates.
 * Postgres accepts an optional `whereExpr`; sort columns are still validated
 * server-side.
 */
export function fetchTableData(
  serverId: number,
  database: string,
  request: TableDataRequest,
): Promise<QueryResult> {
  return call<QueryResult>(RustCommand.FetchTableData, {
    serverId,
    database,
    request,
  });
}
