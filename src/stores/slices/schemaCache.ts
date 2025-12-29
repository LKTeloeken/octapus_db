import { getPostgreStructure } from "@/api/postgres/methods";
import type { DatabaseStructure } from "@/shared/models/database.types";
import type { StateCreator } from "zustand";

interface CacheEntry {
  structure: DatabaseStructure;
  fetchedAt: number;
}

export interface SchemaCacheState {
  // Key format: "serverId:databaseName"
  cache: Record<string, CacheEntry>;
  loading: Record<string, boolean>;

  // Actions
  fetchStructure: (
    serverId: number,
    databaseName: string,
    forceRefresh?: boolean
  ) => Promise<DatabaseStructure>;
  getStructure: (
    serverId: number,
    databaseName: string
  ) => DatabaseStructure | null;
  invalidate: (serverId: number, databaseName?: string) => void;
  clearAll: () => void;
}

const CACHE_TTL_MS = 24 * 60 * 60 * 1000; // 24 hours

const getCacheKey = (serverId: number, databaseName: string) =>
  `${serverId}:${databaseName}`;

export const createSchemaCacheSlice: StateCreator<SchemaCacheState> = (
  set,
  get
) => ({
  cache: {},
  loading: {},

  fetchStructure: async (serverId, databaseName, forceRefresh = false) => {
    const key = getCacheKey(serverId, databaseName);
    const existing = get().cache[key];

    // Return cached if valid and not forcing refresh
    if (
      !forceRefresh &&
      existing &&
      Date.now() - existing.fetchedAt < CACHE_TTL_MS
    ) {
      return existing.structure;
    }

    // Avoid duplicate fetches
    if (get().loading[key]) {
      // Wait for existing fetch
      return new Promise((resolve) => {
        const interval = setInterval(() => {
          const cached = get().cache[key];
          if (cached && !get().loading[key]) {
            clearInterval(interval);
            resolve(cached.structure);
          }
        }, 100);
      });
    }

    set((state) => ({ loading: { ...state.loading, [key]: true } }));

    try {
      const structure = await getPostgreStructure(serverId, databaseName);

      set((state) => ({
        cache: {
          ...state.cache,
          [key]: { structure, fetchedAt: Date.now() },
        },
        loading: { ...state.loading, [key]: false },
      }));

      return structure;
    } catch (error) {
      set((state) => ({ loading: { ...state.loading, [key]: false } }));
      throw error;
    }
  },

  getStructure: (serverId, databaseName) => {
    const key = getCacheKey(serverId, databaseName);
    return get().cache[key]?.structure ?? null;
  },

  invalidate: (serverId, databaseName) => {
    set((state) => {
      const newCache = { ...state.cache };
      Object.keys(newCache).forEach((key) => {
        if (
          databaseName
            ? key === getCacheKey(serverId, databaseName)
            : key.startsWith(`${serverId}:`)
        ) {
          delete newCache[key];
        }
      });
      return { cache: newCache };
    });
  },

  clearAll: () => set({ cache: {}, loading: {} }),
});
