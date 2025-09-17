import { useState } from "react";
import { runPostgreQuery } from "@/api/postgreMethods";

export function useRunQuery() {
  const [loading, setLoading] = useState(false);

  const runQuery = async (
    serverId: number,
    db: string | undefined,
    query: string
  ) => {
    setLoading(true);
    const result = await runPostgreQuery(serverId, query, db);
    setLoading(false);

    return result;
  };

  return { runQuery, loading };
}
