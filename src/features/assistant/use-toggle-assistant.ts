import { useCallback } from 'react';
import { useDbCapabilities } from '@/queries/use-capabilities';
import { useServers } from '@/queries/use-servers';
import { useAssistantStore } from '@/stores/assistant-store';
import { useTabsStore } from '@/stores/tabs-store';

/**
 * Abre ou fecha o painel (botão da barra de abas e `Cmd/Ctrl+J`). Abrir leva o
 * banco da aba ativa como alvo, se o assistente atende esse tipo (C-01).
 */
export const useToggleAssistant = () => {
  const { data: servers } = useServers();
  const capsByType = useDbCapabilities();
  const openPanel = useAssistantStore(state => state.openPanel);
  const closePanel = useAssistantStore(state => state.closePanel);

  return useCallback(() => {
    if (useAssistantStore.getState().panelOpen) {
      closePanel();
      return;
    }
    const { tabs, activeTabId } = useTabsStore.getState();
    const tab = activeTabId ? tabs.get(activeTabId) : undefined;
    const server = servers?.find(item => item.id === tab?.serverId);
    const supported = server
      ? capsByType[server.dbType]?.supportsAssistant === true
      : false;
    openPanel(
      tab && supported
        ? { serverId: tab.serverId, database: tab.database, schema: tab.schema }
        : null,
    );
  }, [servers, capsByType, openPanel, closePanel]);
};
