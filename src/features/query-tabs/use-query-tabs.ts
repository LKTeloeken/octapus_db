import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { DatabaseType } from '@/api/types/server.types';
import { forgetEditorDocument } from '@/components/query-editor/query-editor/use-code-mirror-view';
import { countPendingChanges } from '@/components/results-table/pending-changes';
import { useShortcuts } from '@/lib/keybindings/use-shortcuts';
import { useToggleAssistant } from '@/features/assistant/use-toggle-assistant';
import { useAssistantStatus } from '@/queries/use-assistant-status';
import { useServers } from '@/queries/use-servers';
import { useAssistantStore } from '@/stores/assistant-store';
import { useCommandPaletteStore } from '@/stores/command-palette-store';
import { useGridSearchStore } from '@/stores/grid-search-store';
import { usePendingChangesStore } from '@/stores/pending-changes-store';
import { useQueryResultsStore } from '@/stores/query-results-store';
import { useActiveTab, useTabsStore } from '@/stores/tabs-store';
import type { ClosingTab } from './query-tabs.types';
import { tabDescriptions, tabLocation } from './tab-labels';
import { useTabReorder } from './use-tab-reorder';

export const useQueryTabs = () => {
  const tabs = useTabsStore(state => state.tabs);
  const activeTabId = useTabsStore(state => state.activeTabId);
  const setActiveTab = useTabsStore(state => state.setActiveTab);
  const closeTabInStore = useTabsStore(state => state.closeTab);
  const openQueryTab = useTabsStore(state => state.openQueryTab);
  const moveTab = useTabsStore(state => state.moveTab);
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

  // Mesmo título em schemas, bancos ou servidores diferentes: a aba ganha o
  // trecho que a distingue das outras; todas mostram o caminho no title.
  const serverNames = useMemo(
    () =>
      new Map<number, string>(
        (servers ?? []).map(server => [server.id, server.name]),
      ),
    [servers],
  );
  const descriptions = useMemo(
    () => tabDescriptions(tabsList, id => serverNames.get(id) ?? ''),
    [tabsList, serverNames],
  );
  const locationOf = useCallback(
    (tab: (typeof tabsList)[number]) =>
      tabLocation(tab, serverNames.get(tab.serverId) ?? ''),
    [serverNames],
  );

  // Fecha de vez (o store descarta as pendências da grade junto)
  const discardAndCloseTab = useCallback(
    (id: string) => {
      closeTabInStore(id);
      clearRun(id); // drop the ephemeral editor result of the closed tab
      clearGridSearch(id);
      forgetEditorDocument(id); // texto, seleção e undo guardados do editor
    },
    [closeTabInStore, clearRun, clearGridSearch],
  );

  // Fechar uma aba com alterações pendentes na grade pergunta antes ("x",
  // clique do meio e Cmd/Ctrl+W); sem pendências, fecha na hora.
  const [closing, setClosing] = useState<ClosingTab | null>(null);

  const closeTab = useCallback(
    (id: string) => {
      const pending = usePendingChangesStore.getState().byScope.get(id);
      const tab = tabs.get(id);
      if (!pending || !tab) {
        discardAndCloseTab(id);
        return;
      }
      setClosing({
        tabId: id,
        title: tab.title,
        pendingCount: countPendingChanges(pending),
        open: true,
      });
    },
    [tabs, discardAndCloseTab],
  );

  const confirmClose = useCallback(() => {
    if (closing) discardAndCloseTab(closing.tabId);
    setClosing(prev => prev && { ...prev, open: false });
  }, [closing, discardAndCloseTab]);

  // Mantém o resto para o diálogo sair animado com o texto no lugar
  const cancelClose = useCallback(
    () => setClosing(prev => prev && { ...prev, open: false }),
    [],
  );

  // A aba ativa fica sempre à vista: a lista rola ao trocar de aba e quando
  // encolhe (janela, assistente). Rola só a lista — o `scrollIntoView` também
  // rolaria os ancestrais com overflow escondido.
  const tabListRef = useRef<HTMLDivElement>(null);
  const { drag, onPointerDown: onTabPointerDown } = useTabReorder(
    tabListRef,
    moveTab,
  );
  useEffect(() => {
    const list = tabListRef.current;
    if (!list) return;
    const reveal = () => {
      const tab = list.querySelector<HTMLElement>(
        '[role=tab][data-state=active]',
      )?.parentElement;
      if (!tab) return;
      const bounds = list.getBoundingClientRect();
      const rect = tab.getBoundingClientRect();
      if (rect.left < bounds.left) list.scrollLeft -= bounds.left - rect.left;
      else if (rect.right > bounds.right)
        list.scrollLeft += rect.right - bounds.right;
    };
    reveal();
    if (typeof ResizeObserver === 'undefined') return;
    const observer = new ResizeObserver(reveal);
    observer.observe(list);
    return () => observer.disconnect();
  }, [activeTabId, tabsList.length]);

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
    tabListRef,
    activeTab,
    activeTabId,
    dbTypeByServer,
    descriptions,
    locationOf,
    drag,
    onTabPointerDown,
    setActiveTab,
    closeTab,
    closing,
    confirmClose,
    cancelClose,
    newQueryTab,
    openPalette,
    assistantAvailable: assistantStatus?.configured === true,
    assistantOpen,
    toggleAssistant,
  };
};
