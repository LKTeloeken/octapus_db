import { useState } from "react";
// import { executeQuery } from "@/api/postgres/methods";
import { executeQuery } from "@/api/database/database-methods";

export function useRunQuery() {
  const [loading, setLoading] = useState<boolean>(false);

  const runQuery = async (
    serverId: number,
    database: string,
    query: string,
  ) => {
    setLoading(true);

    try {
      const result = await executeQuery(serverId, database, query);
      setLoading(false);

      return result;
    } catch (error) {
      setLoading(false);
      return Promise.reject(error);
    }
  };

  return { runQuery, loading };
}
