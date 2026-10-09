import { useQueryClient } from '@tanstack/react-query';
import { useCallback, useEffect, useMemo, useState } from 'react';
import toast from 'react-hot-toast';
import { connect } from '@/api/connection';
import type { Server } from '@/api/types/server.types';
import { catalogStatusQuery, hasCatalog, useCatalogSearch } from '@/queries/use-catalog';
import { useServers } from '@/queries/use-servers';
import { useCommandPaletteStore } from '@/stores/command-palette-store';
import { useConnectionStore } from '@/stores/connection-store';
import { useRecentTablesStore, type TableRef } from '@/stores/recent-tables-store';
import { useTabsStore } from '@/stores/tabs-store';
import { fuzzyMatch } from '@/lib/fuzzy';
import { encodeNodeId } from '@/lib/node-ref';
import type {
  PaletteItem,
  PaletteRow,
  ResultGroup,
  QueryCaret,
  TableEntry,
} from './command-palette.types';
import {
  catalogHitToItem,
  flattenGroups,
  groupByServer,
  inPinnedSchema,
  pinnedTableItem,
  resolvePin,
  tableEntry,
} from './palette-items';
import { useTableIndex } from './use-table-index';

const RECENT_BOOST = 50;

export const useCommandPalette = () => {
  const open = useCommandPaletteStore(state => state.isOpen);
  const setOpen = useCommandPaletteStore(state => state.setOpen);
  const togglePalette = useCommandPaletteStore(state => state.toggle);
  const pinnedSchema = useCommandPaletteStore(state => state.pinnedSchema);
  const setPinnedSchema = useCommandPaletteStore(state => state.setPinnedSchema);
  const [query, setQuery] = useState('');
  const [caret, setCaret] = useState<QueryCaret | null>(null);
  const [connectingId, setConnectingId] = useState<string | null>(null);
  const queryClient = useQueryClient();

  const liveEntries = useTableIndex(open);
  const recents = useRecentTablesStore(state => state.recents);
  const addRecent = useRecentTablesStore(state => state.addRecent);
  const { data: servers } = useServers();
  const catalogSearch = useCatalogSearch(query, {
    enabled: open,
    schema: pinnedSchema,
  });

  const serversById = useMemo(
    () => new Map((servers ?? []).map(server => [server.id, server])),
    [servers],
  );

  // Toggle with Cmd/Ctrl+K
  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey)) return;
      if (event.key.toLowerCase() !== 'k') return;

      event.preventDefault();
      togglePalette();
    };

    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [togglePalette]);

  // A busca do catálogo enxerga os catálogos abertos no backend: ao abrir a
  // paleta, os dos databases em uso (abas e recentes) entram — do disco, na
  // hora, e revalidam em segundo plano.
  useEffect(() => {
    if (!open || !servers) return;
    const withCatalog = new Set(
      servers.filter(server => hasCatalog(server.dbType)).map(server => server.id),
    );
    const targets = new Map<string, { serverId: number; database: string }>();
    const add = (serverId: number, database: string) => {
      if (withCatalog.has(serverId)) {
        targets.set(`${serverId}|${database}`, { serverId, database });
      }
    };
    useTabsStore.getState().tabs.forEach(tab => add(tab.serverId, tab.database));
    recents.forEach(ref => add(ref.serverId, ref.database));

    targets.forEach(({ serverId, database }) => {
      void queryClient.prefetchQuery(catalogStatusQuery(serverId, database));
    });
  }, [open, servers, recents, queryClient]);

  // A paleta continua visível durante a animação de saída, então nada muda ao
  // fechar: o campo é zerado ao ABRIR (ainda no render, antes de pintar) e,
  // fechada, ela segue com o último índice em vez de cair para a lista vazia.
  const [wasOpen, setWasOpen] = useState(open);
  const [lastEntries, setLastEntries] = useState(liveEntries);
  if (open !== wasOpen) {
    setWasOpen(open);
    if (open) setQuery('');
  }
  if (open && liveEntries !== lastEntries) setLastEntries(liveEntries);
  const entries = open ? liveEntries : lastEntries;

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

    // Schema fixado: só as relações dele (a busca vazia lista todas)
    if (pinnedSchema !== null) {
      const catalogItems = (catalogSearch.data ?? [])
        .map(hit => {
          const server = serversById.get(hit.serverId);
          // Até a busca nova chegar, a anterior segue na tela (placeholder):
          // dela, só o que é do schema fixado
          const inSchema =
            hit.schema?.toLowerCase() === pinnedSchema.toLowerCase();
          if (!server || !inSchema) return null;
          const entry = tableEntry(server, hit.database, hit.schema, hit.name);
          return pinnedTableItem(entry, trimmed);
        })
        .filter((item): item is PaletteItem => item !== null);

      const localItems = entries
        .filter(entry => inPinnedSchema(entry, pinnedSchema))
        .map(entry => ({ entry, match: fuzzyMatch(trimmed, entry.table) }))
        .filter(item => item.match.matched)
        .sort((a, b) => b.match.score - a.match.score)
        .map(({ entry }) => pinnedTableItem(entry, trimmed));

      return groupByServer([...catalogItems, ...localItems]);
    }

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
        const entry = entriesById.get(id) ?? synthesizeEntry(ref, serversById);
        return {
          key: entry.id,
          target: { kind: 'table', entry },
          label: entry.label,
          indices: [],
          subtitle: `${entry.serverName} · ${entry.database}`,
        };
      });

      return items.length > 0
        ? [{ key: 'recents', heading: 'Recentes', items }]
        : [];
    }

    // Bancos com catálogo: o backend já devolve os melhores, agrupados por nome
    const catalogItems = (catalogSearch.data ?? [])
      .map(hit => catalogHitToItem(hit, trimmed, serversById.get(hit.serverId)))
      .filter((item): item is PaletteItem => item !== null);

    // Demais bancos: fuzzy local sobre a estrutura em cache
    const localItems = entries
      .map(entry => {
        const match = fuzzyMatch(trimmed, entry.label);
        const score = recentEntryIds.has(entry.id)
          ? match.score + RECENT_BOOST
          : match.score;
        return { entry, match, score };
      })
      .filter(item => item.match.matched)
      .sort((a, b) => b.score - a.score)
      .map(
        ({ entry, match }): PaletteItem => ({
          key: entry.id,
          target: { kind: 'table', entry },
          label: entry.label,
          indices: match.indices,
          subtitle: entry.database,
        }),
      );

    return groupByServer([...catalogItems, ...localItems]);
  }, [
    query,
    pinnedSchema,
    entries,
    recents,
    recentEntryIds,
    serversById,
    catalogSearch.data,
  ]);

  // Flatten groups into a single row list so one virtualizer can scroll the
  // whole palette (headings interleaved with their items).
  const rows = useMemo<PaletteRow[]>(() => flattenGroups(groups), [groups]);

  const hasResults = rows.some(row => row.kind === 'item');

  const openEntry = useCallback(
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
    [addRecent, setOpen],
  );

  /** Reescreve a busca e diz onde o cursor fica (o campo obedece). */
  const refine = useCallback((text: string, position: number) => {
    setQuery(text);
    setCaret({ position, nonce: Date.now() });
  }, []);

  const selectItem = useCallback(
    (item: PaletteItem) => {
      const { target } = item;
      switch (target.kind) {
        case 'table':
          void openEntry(target.entry);
          return;
        case 'group':
          // Em vários schemas: `.orders` com o cursor antes do ponto, para
          // digitar o schema
          if (target.schemas.total === 1) {
            const server = serversById.get(target.serverId);
            if (server) {
              void openEntry(
                tableEntry(server, target.database, target.schemas.sample[0], target.name),
              );
            }
            return;
          }
          refine(`.${target.name}`, 0);
          return;
        case 'schema':
          // Escolher um schema fixa ele, como o Tab
          setPinnedSchema(target.schema);
          refine('', 0);
          return;
      }
    },
    [openEntry, refine, serversById, setPinnedSchema],
  );

  /** Tab: fixa um schema (ver `resolvePin`); o resto do texto segue na busca */
  const pinSchema = useCallback(
    (active: PaletteItem | null) => {
      const pin = resolvePin(query, active);
      if (!pin) return;
      setPinnedSchema(pin.schema);
      refine(pin.rest, pin.rest.length);
    },
    [query, refine, setPinnedSchema],
  );

  const unpinSchema = useCallback(() => setPinnedSchema(null), [setPinnedSchema]);

  const isEmptyCache =
    entries.length === 0 &&
    recents.length === 0 &&
    !(servers ?? []).some(server => hasCatalog(server.dbType));

  return {
    open,
    setOpen,
    query,
    setQuery,
    caret,
    rows,
    hasResults,
    connectingId,
    selectItem,
    isEmptyCache,
    pinnedSchema,
    pinSchema,
    unpinSchema,
    isSearching: catalogSearch.isFetching,
  };
};

function synthesizeEntry(
  ref: TableRef,
  serversById: Map<number, Server>,
): TableEntry {
  const server = serversById.get(ref.serverId) ?? {
    id: ref.serverId,
    name: `Servidor ${ref.serverId}`,
    dbType: 'postgres' as const,
  };
  return tableEntry(server, ref.database, ref.schema, ref.table);
}
