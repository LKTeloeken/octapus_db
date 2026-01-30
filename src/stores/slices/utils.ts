import type { DatabaseStructure } from "@/shared/models/database.types";

// 24 hours TTL for structure cache
export const CACHE_TTL_MS = 24 * 60 * 60 * 1000;

// Map to store pending Promises and avoid duplicate requests
export const pendingRequests = new Map<string, Promise<DatabaseStructure>>();

export const getCacheKey = (serverId: number, databaseName: string) =>
  `${serverId}:${databaseName}`;
