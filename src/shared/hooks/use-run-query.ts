import { runPostgreQuery } from "@/api/postgreMethods";

export function useRunQuery() {
  const runQuery = async (
    serverId: number,
    db: string | undefined,
    query: string
  ) => {
    const result = await runPostgreQuery(serverId, query, db);
    return result;
  };

  return { runQuery };
}
