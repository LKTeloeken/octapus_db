import { useCallback, useEffect, useMemo } from 'react';
import type { DatabaseType } from '@/api/types/server.types';
import { forgetEditorDocument } from '@/components/query-editor/query-editor/use-code-mirror-view';
import { useToggleAssistant } from '@/features/assistant/use-toggle-assistant';
import { useAssistantStatus } from '@/queries/use-assistant-status';
import { useServers } from '@/queries/use-servers';
import { useAssistantStore } from '@/stores/assistant-store';
import { useCommandPaletteStore } from '@/stores/command-palette-store';
import { useGridSearchStore } from '@/stores/grid-search-store';
import { useQueryResultsStore } from '@/stores/query-results-store';
import { useActiveTab, useTabsStore } from '@/stores/tabs-store';

export type QueryTabShortcut =
  | { kind: 'close' }
  | { kind: 'goto'; index: number };

/**
 * Cmd/Ctrl+W fecha a aba ativa; Cmd/Ctrl+1..9 vai à N-ésima. Evento já
 * tratado fica de fora: com o foco no assistente, o Cmd/Ctrl+W é das
 * conversas (ver `features/assistant/assistant-shortcuts.ts`).
 */
export function queryTabShortcut(
  event: Pick<
    KeyboardEvent,
    'key' | 'metaKey' | 'ctrlKey' | 'defaultPrevented'
  >,
): QueryTabShortcut | null {
  if (event.defaultPrevented) return null;
  if (!(event.metaKey || event.ctrlKey)) return null;
  if (event.key.toLowerCase() === 'w') return { kind: 'close' };
  if (/^[1-9]$/.test(event.key)) {
    return { kind: 'goto', index: Number(event.key) - 1 };
  }
  return null;
}

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

  // Cmd/Ctrl+W closes the active tab; Cmd/Ctrl+1..9 jumps to the Nth tab
  useEffect(() => {
    const handleShortcut = (event: KeyboardEvent) => {
      const action = queryTabShortcut(event);
      if (!action) return;

      if (action.kind === 'close') {
        event.preventDefault();
        if (activeTabId) closeTab(activeTabId);
        return;
      }

      const target = tabsList[action.index];
      if (target) {
        event.preventDefault();
        setActiveTab(target.id);
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
    assistantAvailable: assistantStatus?.configured === true,
    assistantOpen,
    toggleAssistant,
  };
};
