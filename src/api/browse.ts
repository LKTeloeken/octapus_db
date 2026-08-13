import { call } from './client';
import { RustCommand } from './commands';
import type { TableDataRequest } from './types/browse.types';
import type { QueryResult } from './types/query.types';

/**
 * Server-side browse: the backend builds the SQL/find/scan, validates
 * columns and paginates — never assemble browse queries on the front.
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
