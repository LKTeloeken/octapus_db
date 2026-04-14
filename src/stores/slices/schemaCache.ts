import {
  getSchemasWithTables,
  getColumns,
} from '@/api/database/database-methods';
import { getCacheKey, pendingRequests, CACHE_TTL_MS } from './utils';
import type { DatabaseStructure } from '@/shared/models/database.types';
import type { StateCreator } from 'zustand';
import type { SchemaCacheState } from './schemaCache.types';

export const createSchemaCacheSlice: StateCreator<SchemaCacheState> = (
  set,
  get,
) => ({
  cache: {},
  loading: {},
  recentOpenedTables: [],
  viewLayout: 'horizontal',

  fetchStructure: async (serverId, databaseName, forceRefresh = false) => {
    const key = getCacheKey(serverId, databaseName);
    const existing = get().cache[key];

    // Return cache if valid and not forcing refresh
    if (
      !forceRefresh &&
      existing &&
      Date.now() - existing.fetchedAt < CACHE_TTL_MS
    ) {
      return existing.structure;
    }

    // If there's already a pending request, wait for it
    const pendingRequest = pendingRequests.get(key);
    if (pendingRequest) {
      return pendingRequest;
    }

    // Create new Promise for the request
    const request = (async () => {
      set(state => ({ loading: { ...state.loading, [key]: true } }));

      try {
        const structure = await getSchemasWithTables(serverId, databaseName);
        const formattedStructure: DatabaseStructure = { schemas: structure };

        set(state => ({
          cache: {
            ...state.cache,
            [key]: { structure: formattedStructure, fetchedAt: Date.now() },
          },
          loading: { ...state.loading, [key]: false },
        }));

        return formattedStructure;
      } catch (error) {
        set(state => ({ loading: { ...state.loading, [key]: false } }));
        throw error;
      } finally {
        // Remove pending Promise after completion
        pendingRequests.delete(key);
      }
    })();

    // Store the Promise to avoid duplicate requests
    pendingRequests.set(key, request);

    return request;
  },

  fetchColumns: async (serverId, databaseName, schemaName, tableName) => {
    const key = getCacheKey(serverId, databaseName);

    // Fetch columns from the API
    const columns = await getColumns(
      serverId,
      databaseName,
      schemaName,
      tableName,
    );

    // Update the cache with the new columns
    set(state => {
      const cacheEntry = state.cache[key];
      if (!cacheEntry) {
        // No cache entry exists, return without updating
        return state;
      }

      // Create a new structure with updated columns
      const updatedSchemas = cacheEntry.structure.schemas.map(schema => {
        if (schema.name !== schemaName) {
          return schema;
        }

        return {
          ...schema,
          tables: schema.tables.map(table => {
            if (table.name !== tableName) {
              return table;
            }

            return {
              ...table,
              columns: columns.map(col => ({
                name: col.name,
                dataType: col.dataType,
                isNullable: col.isNullable,
                defaultValue: col.defaultValue,
                isPrimaryKey: col.isPrimaryKey,
                isForeignKey: col.isForeignKey,
                foreignKeyTarget: col.foreignKeyTarget ?? null,
              })),
            };
          }),
        };
      });

      return {
        cache: {
          ...state.cache,
          [key]: {
            ...cacheEntry,
            structure: { schemas: updatedSchemas },
          },
        },
      };
    });

    return columns;
  },

  getStructure: (serverId, databaseName) => {
    const key = getCacheKey(serverId, databaseName);
    return get().cache[key]?.structure ?? null;
  },

  invalidate: (serverId, databaseName) => {
    set(state => {
      const newCache = { ...state.cache };

      if (databaseName) {
        // Remove only the specific entry
        const key = getCacheKey(serverId, databaseName);
        delete newCache[key];
        pendingRequests.delete(key);
      } else {
        // Remove all entries for the server
        const prefix = `${serverId}:`;
        Object.keys(newCache).forEach(key => {
          if (key.startsWith(prefix)) {
            delete newCache[key];
            pendingRequests.delete(key);
          }
        });
      }

      return { cache: newCache };
    });
  },

  clearAll: () => {
    pendingRequests.clear();
    set({ cache: {}, loading: {}, recentOpenedTables: [] });
  },

  recordRecentOpenedTable: item => {
    set(state => {
      const existing = state.recentOpenedTables.filter(
        recent => recent.key !== item.key,
      );

      return {
        recentOpenedTables: [
          { ...item, openedAt: Date.now() },
          ...existing,
        ].slice(0, 10),
      };
    });
  },

  setViewLayout: layout => set({ viewLayout: layout }),
});
