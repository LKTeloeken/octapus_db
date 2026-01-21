import { RustMethods } from "../rust-functions";
import { invoke } from "../utils/invokeHandler";
import type * as Database from "@/shared/models/database.types";
import type { GetDatabasesResponse } from "./database-responses.types";

export async function getDatabases(
  serverId: number,
): Promise<Database.Database[]> {
  const databases = await invoke<GetDatabasesResponse[]>(
    RustMethods.GET_DATABASES,
    {
      serverId,
    },
  );

  console.log("databases", databases);

  return databases.map((db) => ({ name: db.name, serverId }));
}

export async function getSchemasWithTables(serverId: number, database: string) {
  const structure = await invoke<any>(RustMethods.GET_SCHEMAS_WITH_TABLES, {
    serverId,
    database,
  });

  console.log("structure", structure);
}
