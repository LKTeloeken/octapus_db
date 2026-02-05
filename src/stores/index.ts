import { create } from 'zustand';
import { persist, createJSONStorage } from 'zustand/middleware';
import { createSchemaCacheSlice } from './slices/schemaCache';
import {
  indexedDBStorage,
  INDEXED_DB_STORAGE_KEY,
} from './storage/indexed-db-storage';
import type { SchemaCacheState } from './slices/schemaCache.types';

export const useStore = create<SchemaCacheState>()(
  persist(
    (...a) => ({
      ...createSchemaCacheSlice(...a),
    }),
    {
      name: INDEXED_DB_STORAGE_KEY,
      version: 1,
      partialize: state => ({ cache: state.cache }),
      storage: createJSONStorage(() => indexedDBStorage),
    },
  ),
);
