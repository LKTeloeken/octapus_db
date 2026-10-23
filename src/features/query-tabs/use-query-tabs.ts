import { useCallback, useMemo } from 'react';
import type { DatabaseType } from '@/api/types/server.types';
import { forgetEditorDocument } from '@/components/query-editor/query-editor/use-code-mirror-view';
import { useShortcuts } from '@/lib/keybindings/use-shortcuts';
import { useToggleAssistant } from '@/features/assistant/use-toggle-assistant';
import { useAssistantStatus } from '@/queries/use-assistant-status';
import { useServers } from '@/queries/use-servers';
import { useAssistantStore } from '@/stores/assistant-store';
import { useCommandPaletteStore } from '@/stores/command-palette-store';
import { useGridSearchStore } from '@/stores/grid-search-store';
import { useQueryResultsStore } from '@/stores/query-results-store';
import { useActiveTab, useTabsStore } from '@/stores/tabs-store';

export const useQueryTabs = () => {
  const tabs = useTabsStore(state => state.tabs);
  const activeTabId = useTabsStore(state => state.activeTabId);
  const setActiveTab = useTabsStore(state => state.setActiveTab);
  const closeTabInStore = useTabsStore(state => state.closeTab);
  const openQueryTab = useTabsStore(state => state.openQueryTab);
  const clearRun = useQueryResultsStore(state => state.clearRun);
  const clearGridSearch = useGridSearchStore(state => state.clear);
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
      clearGridSearch(id);
      forgetEditorDocument(id); // texto, seleção e undo guardados do editor
    },
    [closeTabInStore, clearRun, clearGridSearch],
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

  // Assistente: o botão só existe no build com a API (C-19)
  const { data: assistantStatus } = useAssistantStatus();
  const assistantOpen = useAssistantStore(state => state.panelOpen);
  const toggleAssistant = useToggleAssistant();

  // Fechar a aba ativa e ir à N-ésima (Cmd/Ctrl+W e Cmd/Ctrl+1..9). Evento
  // já tratado fica de fora: com o foco no assistente, o Cmd/Ctrl+W é das
  // conversas (ver `features/assistant/assistant-shortcuts.ts`).
  const gotoTab = (index: number) => () => {
    const target = tabsList[index];
    if (!target) return false;
    setActiveTab(target.id);
  };
  useShortcuts({
    'tabs.close': () => {
      if (activeTabId) closeTab(activeTabId);
    },
    'tabs.goto1': gotoTab(0),
    'tabs.goto2': gotoTab(1),
    'tabs.goto3': gotoTab(2),
    'tabs.goto4': gotoTab(3),
    'tabs.goto5': gotoTab(4),
    'tabs.goto6': gotoTab(5),
    'tabs.goto7': gotoTab(6),
    'tabs.goto8': gotoTab(7),
    'tabs.goto9': gotoTab(8),
  });

  return {
    tabs: tabsList,
    activeTab,
    activeTabId,
    dbTypeByServer,
    setActiveTab,
    closeTab,
    newQueryTab,
    openPalette,
    assistantAvailable: assistantStatus?.configured === true,
    assistantOpen,
    toggleAssistant,
  };
};
