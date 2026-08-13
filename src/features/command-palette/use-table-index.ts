import { useQueryClient } from '@tanstack/react-query';
import { useMemo } from 'react';
import type { AdapterCapabilities } from '@/api/types/capabilities.types';
import type { DatabaseStructure } from '@/api/types/structure.types';
import { useServers } from '@/queries/use-servers';
import { queryKeys } from '@/queries/keys';
import { encodeNodeId } from '@/lib/node-ref';
import type { TableEntry } from './command-palette.types';

/**
 * Flattens every cached `['structure', serverId, database]` entry into a flat
 * list of tables, cross-referenced with the registered servers. Reads the
 * cache as a snapshot when the palette opens — nothing is fetched from the
 * network, so only databases the user has already browsed appear.
 */
export const useTableIndex = (enabled: boolean): TableEntry[] => {
  const queryClient = useQueryClient();
  const { data: servers } = useServers();

  return useMemo(() => {
    if (!enabled || !servers?.length) return [];

    const serversById = new Map(servers.map(server => [server.id, server]));
    const entries: TableEntry[] = [];

    const cached = queryClient.getQueriesData<DatabaseStructure>({
      queryKey: ['structure'],
    });

    for (const [queryKey, structure] of cached) {
      if (!structure) continue;

      const serverId = queryKey[1] as number;
      const database = queryKey[2] as string;
      const server = serversById.get(serverId);
      if (!server) continue;

      const caps = queryClient.getQueryData<AdapterCapabilities>(
        queryKeys.capabilities(serverId),
      );
      const hasSchemas = caps?.hasSchemas ?? server.dbType === 'postgres';

      for (const schema of structure.schemas) {
        for (const table of schema.tables) {
          const schemaRef = hasSchemas ? schema.name : null;
          const label = hasSchemas
            ? `${schema.name}.${table.name}`
            : table.name;

          entries.push({
            id: encodeNodeId({
              serverId,
              database,
              schema: schemaRef ?? undefined,
              table: table.name,
            }),
            serverId,
            serverName: server.name,
            dbType: server.dbType,
            database,
            schema: schemaRef,
            table: table.name,
            label,
          });
        }
      }
    }

    return entries;
    // queryClient is stable; re-snapshot when the palette opens or servers change
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [enabled, servers]);
};
