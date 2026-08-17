import { useQueryClient } from '@tanstack/react-query';
import { useCallback, useMemo } from 'react';
import { listTables } from '@/api/structure';
import type { DatabaseStructure } from '@/api/types/structure.types';
import { decodeNodeId, nodeKind } from '@/lib/node-ref';
import { useTreeStore } from '@/stores/tree-store';
import { queryKeys } from './keys';

/**
 * Refresh manual do cache de estrutura (o cache mora todo aqui no front, com
 * 24h de staleTime e persistência em IndexedDB — sem isto só resta esperar).
 *
 * Regra importante: colunas são caras em alguns bancos e só podem ser buscadas
 * quando o usuário abre a tabela. Por isso os refreshes de server/database/
 * schema **colapsam** as tabelas do escopo e marcam as colunas como stale com
 * `refetchType: 'none'` — nunca disparam `list_columns` sozinhos. `removeQueries`
 * não serve: numa query montada ela refetcharia na hora.
 */
export function useRefreshStructure() {
  const queryClient = useQueryClient();

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

      await Promise.all([
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
    [collapseTablesIn, queryClient, staleColumnsIn],
  );

  /** Estrutura completa de um database (schemas + tabelas) */
  const refreshDatabase = useCallback(
    async (serverId: number, database: string) => {
      collapseTablesIn({ serverId, database });

      await Promise.all([
        queryClient.invalidateQueries({
          queryKey: queryKeys.structure(serverId, database),
        }),
        staleColumnsIn(serverId, database),
      ]);
    },
    [collapseTablesIn, queryClient, staleColumnsIn],
  );

  /**
   * Só as tabelas de um schema: busca `list_tables` e costura o resultado dentro
   * do `DatabaseStructure` já cacheado, sem tocar nos outros schemas.
   */
  const refreshSchema = useCallback(
    async (serverId: number, database: string, schema: string) => {
      collapseTablesIn({ serverId, database, schema });
      await staleColumnsIn(serverId, database, schema);

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
                    })),
                  }
                : item,
            ),
            fetchedAt: Date.now(),
          },
      );
    },
    [collapseTablesIn, queryClient, staleColumnsIn],
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
