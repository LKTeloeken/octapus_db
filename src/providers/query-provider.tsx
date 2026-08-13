import { QueryClient } from '@tanstack/react-query';
import { createAsyncStoragePersister } from '@tanstack/query-async-storage-persister';
import { PersistQueryClientProvider } from '@tanstack/react-query-persist-client';
import { del, get, set } from 'idb-keyval';
import type { ReactNode } from 'react';

export const STRUCTURE_STALE_TIME_MS = 24 * 60 * 60 * 1000;

/** Only metadata-shaped queries go to disk — table data is always live */
const PERSISTED_DOMAINS = new Set([
  'servers',
  'capabilities',
  'databases',
  'structure',
  'columns',
  'indexes',
]);

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: 1,
      refetchOnWindowFocus: false,
    },
  },
});

const persister = createAsyncStoragePersister({
  key: 'octapus-query-cache',
  storage: {
    getItem: key => get(key),
    setItem: (key, value) => set(key, value),
    removeItem: key => del(key),
  },
});

export const QueryProvider = ({ children }: { children: ReactNode }) => {
  return (
    <PersistQueryClientProvider
      client={queryClient}
      persistOptions={{
        persister,
        maxAge: STRUCTURE_STALE_TIME_MS,
        dehydrateOptions: {
          shouldDehydrateQuery: query =>
            query.state.status === 'success' &&
            PERSISTED_DOMAINS.has(String(query.queryKey[0])),
        },
      }}
    >
      {children}
    </PersistQueryClientProvider>
  );
};
