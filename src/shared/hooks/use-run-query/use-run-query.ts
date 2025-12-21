import { useState } from "react";
import { executeQuery } from "@/api/postgres/methods";

export function useRunQuery() {
  const [loading, setLoading] = useState<boolean>(false);

  const runQuery = async (
    serverId: number,
    db: string | undefined,
    query: string
  ) => {
    setLoading(true);

    try {
      const result = await executeQuery(serverId, query, db);
      setLoading(false);

      return result;
    } catch (error) {
      setLoading(false);
      return Promise.reject(error);
    }
  };

  return { runQuery, loading };
}
