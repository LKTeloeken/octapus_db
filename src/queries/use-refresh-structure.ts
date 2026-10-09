import { useQueryClient } from '@tanstack/react-query';
import { useCallback, useMemo } from 'react';
import { catalogRefresh } from '@/api/catalog';
import { listTables } from '@/api/structure';
import type { CatalogStatus } from '@/api/types/catalog.types';
import type { Server } from '@/api/types/server.types';
import type { DatabaseStructure } from '@/api/types/structure.types';
import { decodeNodeId, nodeKind } from '@/lib/node-ref';
import { useTreeStore } from '@/stores/tree-store';
import { queryKeys } from './keys';
import { hasCatalog } from './use-catalog';

/**
 * Refresh manual do cache de estrutura (o cache mora todo aqui no front, com
 * 24h de staleTime e persistência em IndexedDB — sem isto só resta esperar).
 *
 * Regra importante: colunas são caras em alguns bancos e só podem ser buscadas
 * quando o usuário abre a tabela. Por isso os refreshes de server/database/
 * schema **colapsam** as tabelas do escopo e marcam as colunas como stale com
 * `refetchType: 'none'` — nunca disparam `list_columns` sozinhos. `removeQueries`
 * não serve: numa query montada ela refetcharia na hora.
 *
 * Bancos com catálogo no backend (Postgres, Mongo, SQLite) não têm estrutura
 * no front: o refresh pede ao backend para revalidar (o database, em segundo
 * plano) ou recarregar (um schema, na hora), e os eventos `catalog-event`
 * invalidam as fatias.
 */
export function useRefreshStructure() {
  const queryClient = useQueryClient();

  const usesCatalog = useCallback(
    (serverId: number) =>
      hasCatalog(
        queryClient
          .getQueryData<Server[]>(queryKeys.servers)
          ?.find(server => server.id === serverId)?.dbType,
      ),
    [queryClient],
  );

  /** Revalida no backend; o novo estado entra na hora, o conteúdo pelos eventos */
  const refreshCatalog = useCallback(
    async (serverId: number, database: string, schema?: string) => {
      const status = await catalogRefresh(serverId, database, schema);
      queryClient.setQueryData<CatalogStatus>(
        queryKeys.catalogStatus(serverId, database),
        status,
      );
      if (schema) {
        await queryClient.invalidateQueries({
          queryKey: queryKeys.catalogScope(serverId, database),
        });
      }
    },
    [queryClient],
  );

  /** Colapsa as tabelas expandidas dentro do escopo informado */
  const collapseTablesIn = useCallback(
    (scope: { serverId: number; database?: string; schema?: string }) => {
      const { expanded, collapseNodes } = useTreeStore.getState();

      const nodeIds = Array.from(expanded).filter(id => {
        const ref = decodeNodeId(id);
        if (nodeKind(ref) !== 'table') return false;
        if (ref.serverId !== scope.serverId) return false;
        if (scope.database != null && ref.database !== scope.database) {
          return false;
        }
        return scope.schema == null || ref.schema === scope.schema;
      });

      if (nodeIds.length > 0) collapseNodes(nodeIds);
    },
    [],
  );

  /** Marca as colunas do escopo como stale sem buscar nada agora */
  const staleColumnsIn = useCallback(
    (serverId: number, database?: string, schema?: string) =>
      queryClient.invalidateQueries({
        queryKey: queryKeys.columnsScope(serverId, database, schema),
        refetchType: 'none',
      }),
    [queryClient],
  );

  /** Servidor inteiro: capabilities, databases e a estrutura de todos os databases */
  const refreshServer = useCallback(
    async (serverId: number) => {
      collapseTablesIn({ serverId });

      // Catálogos abertos deste servidor revalidam no backend
      const openCatalogs = usesCatalog(serverId)
        ? Array.from(
            new Set(
              queryClient
                .getQueriesData<CatalogStatus>({
                  queryKey: ['catalog', serverId],
                })
                .map(([key]) => key[2] as string),
            ),
          )
        : [];

      await Promise.all([
        ...openCatalogs.map(database => refreshCatalog(serverId, database)),
        queryClient.invalidateQueries({
          queryKey: queryKeys.capabilities(serverId),
        }),
        queryClient.invalidateQueries({
          queryKey: queryKeys.databases(serverId),
        }),
        queryClient.invalidateQueries({
          queryKey: queryKeys.structureScope(serverId),
        }),
        staleColumnsIn(serverId),
      ]);
    },
    [
      collapseTablesIn,
      queryClient,
      staleColumnsIn,
      usesCatalog,
      refreshCatalog,
    ],
  );

  /** Estrutura completa de um database (schemas + tabelas) */
  const refreshDatabase = useCallback(
    async (serverId: number, database: string) => {
      collapseTablesIn({ serverId, database });

      if (usesCatalog(serverId)) {
        await Promise.all([
          refreshCatalog(serverId, database),
          staleColumnsIn(serverId, database),
        ]);
        return;
      }

      await Promise.all([
        queryClient.invalidateQueries({
          queryKey: queryKeys.structure(serverId, database),
        }),
        staleColumnsIn(serverId, database),
      ]);
    },
    [
      collapseTablesIn,
      queryClient,
      staleColumnsIn,
      usesCatalog,
      refreshCatalog,
    ],
  );

  /**
   * Só as tabelas de um schema: busca `list_tables` e costura o resultado dentro
   * do `DatabaseStructure` já cacheado, sem tocar nos outros schemas. Com
   * catálogo, o backend recarrega só aquele schema.
   */
  const refreshSchema = useCallback(
    async (serverId: number, database: string, schema: string) => {
      collapseTablesIn({ serverId, database, schema });
      await staleColumnsIn(serverId, database, schema);

      if (usesCatalog(serverId)) {
        await refreshCatalog(serverId, database, schema);
        return;
      }

      const structureKey = queryKeys.structure(serverId, database);
      const cached = queryClient.getQueryData<DatabaseStructure>(structureKey);

      // Schema ainda não presente no cache → não há o que costurar, recarrega
      // o database inteiro.
      if (!cached?.schemas.some(item => item.name === schema)) {
        await queryClient.invalidateQueries({ queryKey: structureKey });
        return;
      }

      const tables = await listTables(serverId, database, schema);

      queryClient.setQueryData<DatabaseStructure>(
        structureKey,
        previous =>
          previous && {
            ...previous,
            schemas: previous.schemas.map(item =>
              item.name === schema
                ? {
                    ...item,
                    tables: tables.map(table => ({
                      name: table.name,
                      tableType: table.tableType,
                      sizeBytes: table.sizeBytes,
                    })),
                  }
                : item,
            ),
            fetchedAt: Date.now(),
          },
      );
    },
    [
      collapseTablesIn,
      queryClient,
      staleColumnsIn,
      usesCatalog,
      refreshCatalog,
    ],
  );

  /** Colunas (e índices) de uma tabela — único caso em que buscar coluna é o pedido */
  const refreshTable = useCallback(
    async (
      serverId: number,
      database: string,
      schema: string | null,
      table: string,
    ) => {
      await Promise.all([
        queryClient.invalidateQueries({
          queryKey: queryKeys.columns(serverId, database, schema ?? '', table),
        }),
        queryClient.invalidateQueries({
          queryKey: queryKeys.indexes(serverId, database, schema ?? '', table),
        }),
      ]);
    },
    [queryClient],
  );

  return useMemo(
    () => ({ refreshServer, refreshDatabase, refreshSchema, refreshTable }),
    [refreshServer, refreshDatabase, refreshSchema, refreshTable],
  );
}
