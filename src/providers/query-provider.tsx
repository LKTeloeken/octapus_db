import { MutationCache, QueryCache, QueryClient } from '@tanstack/react-query';
import { createAsyncStoragePersister } from '@tanstack/query-async-storage-persister';
import { PersistQueryClientProvider } from '@tanstack/react-query-persist-client';
import { del, get, set } from 'idb-keyval';
import type { ReactNode } from 'react';
import { getVaultStatus } from '@/api/vault';
import { useVaultStore } from '@/stores/vault-store';

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

/**
 * Qualquer erro pode ser "o cofre está trancado". Em vez de casar o texto da
 * mensagem — o backend rejeita com string, não com código — perguntamos ao
 * backend qual é o estado real. Se estiver trancado, é essa a causa, e o
 * diálogo de senha mestre aparece sozinho.
 */
const openUnlockIfVaultIsLocked = async () => {
  if (useVaultStore.getState().isUnlockOpen) return;

  try {
    const status = await getVaultStatus();
    if (status.locked) useVaultStore.getState().openUnlock();
  } catch {
    // Se nem o status responde, o problema não é o cofre — deixa o erro
    // original seguir para quem chamou.
  }
};

const queryClient = new QueryClient({
  queryCache: new QueryCache({ onError: openUnlockIfVaultIsLocked }),
  mutationCache: new MutationCache({ onError: openUnlockIfVaultIsLocked }),
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
