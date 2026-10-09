import { useQueryClient, type QueryKey } from '@tanstack/react-query';
import { useEffect } from 'react';
import { onCatalogEvent } from '@/api/catalog';
import type { CatalogEvent } from '@/api/types/catalog.types';
import { queryKeys } from './keys';

export interface CatalogInvalidation {
  /** Refetch das queries montadas (fatias visíveis) */
  refetch: QueryKey[];
  /** Só marcar como velhas — colunas são caras e só voltam quando pedidas */
  stale: QueryKey[];
}

/**
 * O que cada evento do catálogo invalida. Fatias montadas são poucas (os nós
 * abertos da árvore), então invalidar o database inteiro é barato e não perde
 * nada: `relations` e `ready` mexem em contagens da lista de schemas e no
 * conteúdo dos schemas recarregados.
 */
export function catalogEventInvalidations(
  event: CatalogEvent,
): CatalogInvalidation {
  const { serverId, database } = event;

  switch (event.type) {
    case 'syncing':
    case 'error':
    case 'cancelled':
      return {
        refetch: [queryKeys.catalogStatus(serverId, database)],
        stale: [],
      };
    case 'schemas':
    case 'relations':
      return {
        refetch: [
          queryKeys.catalogScope(serverId, database),
          queryKeys.catalogSearchScope,
        ],
        stale: [],
      };
    case 'ready':
      return {
        refetch: [
          queryKeys.catalogScope(serverId, database),
          queryKeys.catalogSearchScope,
        ],
        stale: event.changed.map(schema =>
          queryKeys.columnsScope(serverId, database, schema),
        ),
      };
  }
}

/**
 * Liga os eventos do catálogo ao cache do React Query. Montado uma vez, na
 * árvore (o único painel sempre presente do shell).
 */
export function useCatalogEvents() {
  const queryClient = useQueryClient();

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let disposed = false;

    void onCatalogEvent(event => {
      const { refetch, stale } = catalogEventInvalidations(event);
      for (const queryKey of refetch) {
        void queryClient.invalidateQueries({ queryKey });
      }
      for (const queryKey of stale) {
        void queryClient.invalidateQueries({ queryKey, refetchType: 'none' });
      }
    }).then(stop => {
      if (disposed) stop();
      else unlisten = stop;
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [queryClient]);
}
