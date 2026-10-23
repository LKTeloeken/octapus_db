import {
  QueryClient,
  defaultShouldDehydrateMutation,
  type DehydrateOptions,
  type Mutation,
} from '@tanstack/react-query';
import { createAsyncStoragePersister } from '@tanstack/query-async-storage-persister';
import { PersistQueryClientProvider } from '@tanstack/react-query-persist-client';
import { del, get, set } from 'idb-keyval';
import type { ReactNode } from 'react';

export const STRUCTURE_STALE_TIME_MS = 24 * 60 * 60 * 1000;

/**
 * Only metadata-shaped queries go to disk — table data is always live. Nada do
 * assistente entra aqui (RN-01): ver `queries/assistant-keys.test.ts`.
 */
export const PERSISTED_DOMAINS: ReadonlySet<string> = new Set([
  'servers',
  'capabilities',
  'databases',
  'structure',
  'columns',
  'indexes',
]);

/**
 * Mutação pausada (sem rede, ou esperando a vez na fila de um `scope`) vai
 * para o disco com as variáveis — menos as do assistente (RN-01): o
 * comentário de uma avaliação, por exemplo, nunca é gravado.
 */
export function shouldPersistMutation(mutation: Mutation) {
  return (
    mutation.options.mutationKey?.[0] !== 'assistant' &&
    defaultShouldDehydrateMutation(mutation)
  );
}

/**
 * O que o persister grava no IndexedDB: as queries de sucesso dos
 * `PERSISTED_DOMAINS` e as mutações pausadas que não são do assistente. A
 * prova do RN-01 (`persisted-cache.test.ts`) passa o cache por aqui.
 */
export const PERSIST_DEHYDRATE_OPTIONS: DehydrateOptions = {
  shouldDehydrateQuery: query =>
    query.state.status === 'success' &&
    PERSISTED_DOMAINS.has(String(query.queryKey[0])),
  shouldDehydrateMutation: shouldPersistMutation,
};

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
        // Sobe quando o formato do cache persistido muda. `catalog-v1`: os
        // Postgres passaram para o catálogo do backend, e a estrutura inteira
        // que ficava aqui (dezenas de MB num banco multi-tenant) é descartada.
        // `server-groups-v1`: `Server` ganhou o `groupId` (grupos da sidebar).
        // `ssl-capabilities-v1`: `Server` trocou `sslEnabled` por `sslMode` e
        // `AdapterCapabilities` ganhou as flags de catálogo, estrutura e backup.
        // `assistant-capabilities-v1`: `AdapterCapabilities` ganhou
        // `supportsAssistant` e `queryLanguage`. `structure-folders-v1`:
        // ganhou `structureFolders` e `relationFolders` (pastas da árvore).
        // `structure-folders-v2`: a pasta Índices foi para dentro da tabela.
        // `structure-folders-v3`: pastas de sequences, triggers e chaves
        // estrangeiras
        buster: 'structure-folders-v3',
        dehydrateOptions: PERSIST_DEHYDRATE_OPTIONS,
      }}
    >
      {children}
    </PersistQueryClientProvider>
  );
};
