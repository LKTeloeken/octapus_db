import { useCallback, useEffect, useMemo } from 'react';
import type { DatabaseType } from '@/api/types/server.types';
import { useServers } from '@/queries/use-servers';
import { useCommandPaletteStore } from '@/stores/command-palette-store';
import { useQueryResultsStore } from '@/stores/query-results-store';
import { useActiveTab, useTabsStore } from '@/stores/tabs-store';

export const useQueryTabs = () => {
  const tabs = useTabsStore(state => state.tabs);
  const activeTabId = useTabsStore(state => state.activeTabId);
  const setActiveTab = useTabsStore(state => state.setActiveTab);
  const closeTabInStore = useTabsStore(state => state.closeTab);
  const openQueryTab = useTabsStore(state => state.openQueryTab);
  const clearRun = useQueryResultsStore(state => state.clearRun);
  const setPaletteOpen = useCommandPaletteStore(state => state.setOpen);
  const { data: servers } = useServers();

  const activeTab = useActiveTab();
  const tabsList = useMemo(() => Array.from(tabs.values()), [tabs]);

  // Banco de cada servidor: a cor de identidade vai no ícone da aba.
  const dbTypeByServer = useMemo(
    () =>
      new Map<number, DatabaseType>(
        (servers ?? []).map(server => [server.id, server.dbType]),
      ),
    [servers],
  );

  const closeTab = useCallback(
    (id: string) => {
      closeTabInStore(id);
      clearRun(id); // drop the ephemeral editor result of the closed tab
    },
    [closeTabInStore, clearRun],
  );

  // "+" da barra: nova consulta no mesmo servidor/banco da aba ativa.
  const newQueryTab = useCallback(() => {
    if (!activeTab) return;
    openQueryTab({
      serverId: activeTab.serverId,
      database: activeTab.database,
      schema: activeTab.schema,
    });
  }, [activeTab, openQueryTab]);

  const openPalette = useCallback(() => setPaletteOpen(true), [setPaletteOpen]);

  // Cmd/Ctrl+W closes the active tab; Cmd/Ctrl+1..9 jumps to the Nth tab
  useEffect(() => {
    const handleShortcut = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey)) return;

      if (event.key.toLowerCase() === 'w') {
        event.preventDefault();
        if (activeTabId) closeTab(activeTabId);
        return;
      }

      if (/^[1-9]$/.test(event.key)) {
        const target = tabsList[Number(event.key) - 1];
        if (target) {
          event.preventDefault();
          setActiveTab(target.id);
        }
      }
    };

    window.addEventListener('keydown', handleShortcut);
    return () => window.removeEventListener('keydown', handleShortcut);
  }, [activeTabId, closeTab, tabsList, setActiveTab]);

  return {
    tabs: tabsList,
    activeTab,
    activeTabId,
    dbTypeByServer,
    setActiveTab,
    closeTab,
    newQueryTab,
    openPalette,
  };
};
