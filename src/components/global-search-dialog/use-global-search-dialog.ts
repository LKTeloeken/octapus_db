import { useStore } from '@/stores';
import type { TreeNode } from '@/shared/models/database.types';
import { TreeNodeType } from '@/shared/models/database.types';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { SearchTarget } from './global-search-dialog.types';
import toast from 'react-hot-toast';

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
  const { cache, fetchColumns, fetchStructure } = useStore();
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState('');
  const [hydratingColumns, setHydratingColumns] = useState(false);
  const hydratedColumnTables = useRef<Set<string>>(new Set());

  const databaseTargets = useMemo(() => {
    const fromCache = Object.keys(cache).map(parseCacheKey);
    const fromTree = Array.from(nodes.values()).flatMap(node => {
      if (
        node.type !== TreeNodeType.Database ||
        node.metadata.type !== TreeNodeType.Database
      ) {
        return [];
      }

      return [
        {
          serverId: node.metadata.serverId,
          databaseName: node.metadata.databaseName,
        },
      ];
    });
    const fromServers = Array.from(nodes.values()).flatMap(node => {
      if (
        node.type !== TreeNodeType.Server ||
        node.metadata.type !== TreeNodeType.Server
      ) {
        return [];
      }

      return [
        {
          serverId: node.metadata.serverId,
          databaseName: node.metadata.serverData?.default_database ?? 'postgres',
        },
      ];
    });

    const map = new Map<string, { serverId: number; databaseName: string }>();
    [...fromCache, ...fromTree, ...fromServers].forEach(item => {
      map.set(`${item.serverId}:${item.databaseName}`, item);
    });

    return Array.from(map.values());
  }, [cache, nodes]);

  const searchTargets = useMemo(() => {
    const targets: SearchTarget[] = [];

    Object.entries(cache).forEach(([key, cacheEntry]) => {
      const { serverId, databaseName } = parseCacheKey(key);

      cacheEntry.structure.schemas.forEach(schema => {
        schema.tables.forEach(table => {
          targets.push({
            type: 'table',
            serverId,
            databaseName,
            schemaName: schema.name,
            tableName: table.name,
          });

          table.columns?.forEach(column => {
            targets.push({
              type: 'column',
              serverId,
              databaseName,
              schemaName: schema.name,
              tableName: table.name,
              columnName: column.name,
            });
          });
        });
      });
    });

    return targets;
  }, [cache]);

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

    Promise.all(
      databaseTargets.map(target =>
        fetchStructure(target.serverId, target.databaseName).catch(() => null),
      ),
    ).catch(() => null);
  }, [databaseTargets, fetchStructure, open]);

  useEffect(() => {
    if (!open) return;

    const pending: Array<{
      serverId: number;
      databaseName: string;
      schemaName: string;
      tableName: string;
      key: string;
    }> = [];

    Object.entries(cache).forEach(([key, cacheEntry]) => {
      const { serverId, databaseName } = parseCacheKey(key);
      cacheEntry.structure.schemas.forEach(schema => {
        schema.tables.forEach(table => {
          if (table.columns?.length) return;
          const tableKey = `${serverId}:${databaseName}:${schema.name}:${table.name}`;
          if (hydratedColumnTables.current.has(tableKey)) return;
          pending.push({
            serverId,
            databaseName,
            schemaName: schema.name,
            tableName: table.name,
            key: tableKey,
          });
        });
      });
    });

    if (pending.length === 0) return;

    setHydratingColumns(true);
    Promise.all(
      pending.map(async item => {
        hydratedColumnTables.current.add(item.key);
        await fetchColumns(
          item.serverId,
          item.databaseName,
          item.schemaName,
          item.tableName,
        );
      }),
    )
      .catch(() => {
        toast.error('Failed to hydrate columns for global search');
      })
      .finally(() => setHydratingColumns(false));
  }, [cache, fetchColumns, open]);

  const results = useMemo(() => {
    const normalized = search.trim().toLowerCase();
    if (!normalized) return searchTargets.slice(0, 100);

    return searchTargets
      .filter(item => {
        const value = `${item.type} ${item.schemaName}.${item.tableName} ${item.columnName ?? ''}`.toLowerCase();
        return value.includes(normalized);
      })
      .slice(0, 100);
  }, [search, searchTargets]);

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
    hydratingColumns,
    handleSelect,
  };
};
