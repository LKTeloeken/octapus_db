import { useStore } from '@/stores';
import type { TreeNode } from '@/shared/models/database.types';
import { TreeNodeType } from '@/shared/models/database.types';
import { useCallback, useEffect, useMemo, useState } from 'react';
import type { SearchTarget } from './global-search-dialog.types';
import toast from 'react-hot-toast';
import { ensureServerConnection } from '@/api/server/methods';

const MAX_SEARCH_RESULTS = 300;

const parseCacheKey = (key: string) => {
  const [serverId, ...databaseParts] = key.split(':');
  return {
    serverId: Number(serverId),
    databaseName: databaseParts.join(':'),
  };
};

export const useGlobalSearchDialog = (
  nodes: Map<string, TreeNode>,
  onOpenTable: (
    serverId: number,
    databaseName: string,
    schemaName: string,
    tableName: string,
  ) => Promise<void>,
) => {
  const { cache, fetchStructure, recentOpenedTables } = useStore();
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState('');
  const [isOpeningTable, setIsOpeningTable] = useState(false);
  const [isLoadingStructures] = useState(false);

  const serverInfo = useMemo(() => {
    const connectedServerIds = new Set<number>();
    const serverNameById = new Map<number, string>();

    Array.from(nodes.values()).forEach(node => {
      if (
        node.type !== TreeNodeType.Server ||
        node.metadata.type !== TreeNodeType.Server
      ) {
        return;
      }

      serverNameById.set(node.metadata.serverId, node.name);
      if (node.isConnected) {
        connectedServerIds.add(node.metadata.serverId);
      }
    });

    return {
      connectedServerIds,
      serverNameById,
    };
  }, [nodes]);

  const searchTargets = useMemo(() => {
    const targets: SearchTarget[] = [];

    Object.entries(cache).forEach(([key, cacheEntry]) => {
      const { serverId, databaseName } = parseCacheKey(key);
      const serverName =
        serverInfo.serverNameById.get(serverId) ?? `Server ${serverId}`;

      cacheEntry.structure.schemas.forEach(schema => {
        if (!schema.name?.trim()) return;
        schema.tables.forEach(table => {
          if (!table.name?.trim()) return;
          targets.push({
            serverId,
            serverName,
            databaseName,
            schemaName: schema.name,
            tableName: table.name,
            searchTokens: table.columns?.map(column => column.name) ?? [],
          });
        });
      });
    });

    return targets;
  }, [cache, serverInfo.serverNameById]);

  const searchTargetsByDatabase = useMemo(() => {
    const map = new Map<string, SearchTarget[]>();
    searchTargets.forEach(target => {
      const key = `${target.serverId}:${target.databaseName}`;
      const current = map.get(key) ?? [];
      current.push(target);
      map.set(key, current);
    });
    return map;
  }, [searchTargets]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key.toLowerCase() === 'k' && (event.metaKey || event.ctrlKey)) {
        event.preventDefault();
        setOpen(current => !current);
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  const baseResults = useMemo(() => {
    const normalized = search.trim().toLowerCase();
    if (!normalized) return searchTargets.slice(0, MAX_SEARCH_RESULTS);

    return searchTargets
      .filter(item => {
        const tableRefWithDot = `${item.schemaName}.${item.tableName}`;
        const tableRefWithSlash = `${item.schemaName}/${item.tableName}`;
        const columnTokens = (item.searchTokens ?? []).join(' ');
        const value =
          `${item.serverName} ${item.databaseName} ${item.schemaName} ${item.tableName} ${tableRefWithDot} ${tableRefWithSlash} ${columnTokens}`.toLowerCase();
        return value.includes(normalized);
      })
      .slice(0, MAX_SEARCH_RESULTS);
  }, [search, searchTargets]);

  const results = useMemo(() => {
    const recentKeyOrder = new Map(
      recentOpenedTables.map((item, index) => [item.key, index]),
    );
    const recentKeys = new Set(recentKeyOrder.keys());
    const prioritized = baseResults
      .filter(item =>
        recentKeys.has(
          `${item.serverId}:${item.databaseName}:${item.schemaName}:${item.tableName}`,
        ),
      )
      .sort((a, b) => {
        const aKey = `${a.serverId}:${a.databaseName}:${a.schemaName}:${a.tableName}`;
        const bKey = `${b.serverId}:${b.databaseName}:${b.schemaName}:${b.tableName}`;
        return (recentKeyOrder.get(aKey) ?? 999) - (recentKeyOrder.get(bKey) ?? 999);
      });
    const remaining = baseResults.filter(
      item =>
        !recentKeys.has(
          `${item.serverId}:${item.databaseName}:${item.schemaName}:${item.tableName}`,
        ),
    );

    return [...prioritized, ...remaining].slice(0, MAX_SEARCH_RESULTS);
  }, [baseResults, recentOpenedTables]);

  const groupedResults = useMemo(() => {
    const groups = new Map<string, SearchTarget[]>();

    results.forEach(item => {
      const groupKey = `${item.serverName} / ${item.databaseName} / ${item.schemaName}`;
      const current = groups.get(groupKey) ?? [];
      current.push(item);
      groups.set(groupKey, current);
    });

    return Array.from(groups.entries()).map(([group, items]) => ({
      group,
      items,
    }));
  }, [results]);

  const handleSelect = useCallback(
    async (item: SearchTarget) => {
      setIsOpeningTable(true);
      try {
        if (!serverInfo.connectedServerIds.has(item.serverId)) {
          await ensureServerConnection(item.serverId, item.databaseName);
          await fetchStructure(item.serverId, item.databaseName);
        } else {
          const cacheKey = `${item.serverId}:${item.databaseName}`;
          if (!searchTargetsByDatabase.has(cacheKey)) {
            await fetchStructure(item.serverId, item.databaseName);
          }
        }

        await onOpenTable(
          item.serverId,
          item.databaseName,
          item.schemaName,
          item.tableName,
        );
        setOpen(false);
      } catch (error) {
        toast.error(
          error instanceof Error ? error.message : 'Failed to open selected table',
        );
      } finally {
        setIsOpeningTable(false);
      }
    },
    [
      fetchStructure,
      onOpenTable,
      searchTargetsByDatabase,
      serverInfo.connectedServerIds,
    ],
  );

  return {
    open,
    setOpen,
    search,
    setSearch,
    results,
    groupedResults,
    isLoadingStructures,
    isOpeningTable,
    handleSelect,
  };
};
