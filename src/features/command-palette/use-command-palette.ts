import { useCallback, useEffect, useMemo, useState } from 'react';
import toast from 'react-hot-toast';
import { connect } from '@/api/connection';
import { useServers } from '@/queries/use-servers';
import { useConnectionStore } from '@/stores/connection-store';
import { useRecentTablesStore, type TableRef } from '@/stores/recent-tables-store';
import { useTabsStore } from '@/stores/tabs-store';
import { fuzzyMatch } from '@/lib/fuzzy';
import { encodeNodeId } from '@/lib/node-ref';
import type {
  PaletteItem,
  PaletteRow,
  ResultGroup,
  TableEntry,
} from './command-palette.types';
import { useTableIndex } from './use-table-index';

const RECENT_BOOST = 50;

export const useCommandPalette = () => {
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState('');
  const [connectingId, setConnectingId] = useState<string | null>(null);

  const entries = useTableIndex(open);
  const recents = useRecentTablesStore(state => state.recents);
  const addRecent = useRecentTablesStore(state => state.addRecent);
  const { data: servers } = useServers();

  // Toggle with Cmd/Ctrl+K
  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey)) return;
      if (event.key.toLowerCase() !== 'k') return;

      event.preventDefault();
      setOpen(prev => !prev);
    };

    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, []);

  // Reset the search field whenever the dialog closes
  useEffect(() => {
    if (!open) setQuery('');
  }, [open]);

  const recentEntryIds = useMemo(
    () =>
      new Set(
        recents.map(ref =>
          encodeNodeId({
            serverId: ref.serverId,
            database: ref.database,
            schema: ref.schema ?? undefined,
            table: ref.table,
          }),
        ),
      ),
    [recents],
  );

  const groups = useMemo<ResultGroup[]>(() => {
    const trimmed = query.trim();

    // Empty search → recents only (newest first).
    if (trimmed.length === 0) {
      const entriesById = new Map(entries.map(entry => [entry.id, entry]));

      const items: PaletteItem[] = recents.map(ref => {
        const id = encodeNodeId({
          serverId: ref.serverId,
          database: ref.database,
          schema: ref.schema ?? undefined,
          table: ref.table,
        });
        const entry = entriesById.get(id) ?? synthesizeEntry(ref, servers);
        return {
          entry,
          indices: [],
          subtitle: `${entry.serverName} · ${entry.database}`,
        };
      });

      return items.length > 0
        ? [{ key: 'recents', heading: 'Recentes', items }]
        : [];
    }

    // Active search → fuzzy filter over all cached tables, grouped by server.
    const scored = entries
      .map(entry => {
        const match = fuzzyMatch(trimmed, entry.label);
        const score = recentEntryIds.has(entry.id)
          ? match.score + RECENT_BOOST
          : match.score;
        return { entry, match, score };
      })
      .filter(item => item.match.matched)
      .sort((a, b) => b.score - a.score);

    const byServer = new Map<number, ResultGroup>();
    for (const { entry, match } of scored) {
      let group = byServer.get(entry.serverId);
      if (!group) {
        group = {
          key: `server-${entry.serverId}`,
          heading: entry.serverName,
          items: [],
        };
        byServer.set(entry.serverId, group);
      }
      group.items.push({
        entry,
        indices: match.indices,
        subtitle: entry.database,
      });
    }

    return Array.from(byServer.values());
  }, [query, entries, recents, recentEntryIds, servers]);

  // Flatten groups into a single row list so one virtualizer can scroll the
  // whole palette (headings interleaved with their items).
  const rows = useMemo<PaletteRow[]>(() => {
    const flat: PaletteRow[] = [];
    for (const group of groups) {
      flat.push({ kind: 'header', key: group.key, heading: group.heading });
      for (const item of group.items) {
        flat.push({ kind: 'item', key: item.entry.id, item });
      }
    }
    return flat;
  }, [groups]);

  const hasResults = rows.some(row => row.kind === 'item');

  const selectEntry = useCallback(
    async (entry: TableEntry) => {
      const ref: TableRef = {
        serverId: entry.serverId,
        database: entry.database,
        schema: entry.schema,
        table: entry.table,
      };

      addRecent(ref);

      const tabsState = useTabsStore.getState();
      const alreadyOpen = tabsState.tabs.has(entry.id);
      const connected = useConnectionStore
        .getState()
        .isConnected(entry.serverId, entry.database);

      if (alreadyOpen || connected) {
        tabsState.openBrowseTab(ref);
        setOpen(false);
        return;
      }

      // Server not yet connected — connect first, open only on success.
      setConnectingId(entry.id);
      try {
        await connect(entry.serverId, entry.database);
        useConnectionStore
          .getState()
          .markConnected(entry.serverId, entry.database);
        useTabsStore.getState().openBrowseTab(ref);
        setOpen(false);
      } catch (error) {
        toast.error(error instanceof Error ? error.message : String(error));
      } finally {
        setConnectingId(null);
      }
    },
    [addRecent],
  );

  const isEmptyCache = entries.length === 0 && recents.length === 0;

  return {
    open,
    setOpen,
    query,
    setQuery,
    rows,
    hasResults,
    connectingId,
    selectEntry,
    isEmptyCache,
  };
};

function synthesizeEntry(
  ref: TableRef,
  servers: { id: number; name: string; dbType: TableEntry['dbType'] }[] | undefined,
): TableEntry {
  const server = servers?.find(s => s.id === ref.serverId);
  return {
    id: encodeNodeId({
      serverId: ref.serverId,
      database: ref.database,
      schema: ref.schema ?? undefined,
      table: ref.table,
    }),
    serverId: ref.serverId,
    serverName: server?.name ?? `Servidor ${ref.serverId}`,
    dbType: server?.dbType ?? 'postgres',
    database: ref.database,
    schema: ref.schema,
    table: ref.table,
    label: ref.schema ? `${ref.schema}.${ref.table}` : ref.table,
  };
}
