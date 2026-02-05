import { get, set, del } from 'idb-keyval';
import type { StateStorage } from 'zustand/middleware';

export const INDEXED_DB_STORAGE_KEY = 'octapus-db-store';

/**
 * Adapter to persist Zustand state in IndexedDB using idb-keyval.
 * More efficient than localStorage for large data volumes.
 */
export const indexedDBStorage: StateStorage = {
  getItem: async (name: string): Promise<string | null> => {
    const value = await get<string>(name);
    return value ?? null;
  },

  setItem: async (name: string, value: string): Promise<void> => {
    await set(name, value);
  },

  removeItem: async (name: string): Promise<void> => {
    await del(name);
  },
};
