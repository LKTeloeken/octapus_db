import { useStore } from '@/stores';
import type { TreeNode } from '@/shared/models/database.types';
import { TreeNodeType } from '@/shared/models/database.types';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { SearchTarget } from './global-search-dialog.types';
import toast from 'react-hot-toast';
import { getDatabases } from '@/api/database/database-methods';

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
  const { cache, fetchStructure } = useStore();
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState('');
  const [isLoadingStructures, setIsLoadingStructures] = useState(false);
  const hydratedDatabases = useRef<Set<string>>(new Set());

  const serverTargets = useMemo(
    () =>
      Array.from(nodes.values()).flatMap(node => {
        if (
          node.type !== TreeNodeType.Server ||
          node.metadata.type !== TreeNodeType.Server
        ) {
          return [];
        }

        return [
          {
            serverId: node.metadata.serverId,
            serverName: node.name,
          },
        ];
      }),
    [nodes],
  );

  const searchTargets = useMemo(() => {
    const targets: SearchTarget[] = [];

    Object.entries(cache).forEach(([key, cacheEntry]) => {
      const { serverId, databaseName } = parseCacheKey(key);

      cacheEntry.structure.schemas.forEach(schema => {
        schema.tables.forEach(table => {
          targets.push({
            serverId,
            serverName:
              Array.from(nodes.values()).find(
                node =>
                  node.type === TreeNodeType.Server &&
                  node.metadata.type === TreeNodeType.Server &&
                  node.metadata.serverId === serverId,
              )?.name ?? `Server ${serverId}`,
            databaseName,
            schemaName: schema.name,
            tableName: table.name,
          });
        });
      });
    });

    return targets;
  }, [cache, nodes]);

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

  useEffect(() => {
    if (!open) return;

    setIsLoadingStructures(true);
    Promise.all(
      serverTargets.map(async server => {
        const databases = await getDatabases(server.serverId);

        await Promise.all(
          databases.map(async database => {
            const key = `${server.serverId}:${database.name}`;
            if (hydratedDatabases.current.has(key)) return;
            hydratedDatabases.current.add(key);
            await fetchStructure(server.serverId, database.name).catch(() => null);
          }),
        );
      }),
    )
      .catch(() => {
        toast.error('Failed to load server tables for command search');
      })
      .finally(() => setIsLoadingStructures(false));
  }, [fetchStructure, open, serverTargets]);

  const results = useMemo(() => {
    const normalized = search.trim().toLowerCase();
    if (!normalized) return searchTargets.slice(0, MAX_SEARCH_RESULTS);

    return searchTargets
      .filter(item => {
        const value =
          `${item.serverName} ${item.databaseName} ${item.schemaName} ${item.tableName} ${item.schemaName}/${item.tableName}`.toLowerCase();
        return value.includes(normalized);
      })
      .slice(0, MAX_SEARCH_RESULTS);
  }, [search, searchTargets]);

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
      await onOpenTable(
        item.serverId,
        item.databaseName,
        item.schemaName,
        item.tableName,
      );
      setOpen(false);
    },
    [onOpenTable],
  );

  return {
    open,
    setOpen,
    search,
    setSearch,
    results,
    groupedResults,
    isLoadingStructures,
    handleSelect,
  };
};
