import { useState } from 'react';
import { executeQuery } from '@/api/database/database-methods';
import type { ExecuteQueryOptions } from '@/api/database/database-methods.types';

export function useRunQuery() {
  const [loading, setLoading] = useState<boolean>(false);

  const runQuery = async (
    serverId: number,
    database: string,
    query: string,
    options: Partial<ExecuteQueryOptions> = {},
  ) => {
    setLoading(true);

    try {
      const result = await executeQuery(serverId, database, query, options);

      return result;
    } catch (error) {
      return Promise.reject(error);
    } finally {
      setLoading(false);
    }
  };

  return { runQuery, loading };
}
