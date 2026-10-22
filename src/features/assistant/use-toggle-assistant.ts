import { useCallback } from 'react';
import { useDbCapabilities } from '@/queries/use-capabilities';
import { useServers } from '@/queries/use-servers';
import {
  useAssistantStore,
  type AssistantTarget,
} from '@/stores/assistant-store';
import { useTabsStore } from '@/stores/tabs-store';

/**
 * O banco da aba de query ativa como alvo da conversa, se o assistente atende
 * esse tipo (C-01); senão `null`.
 */
const useActiveTabTarget = () => {
  const { data: servers } = useServers();
  const capsByType = useDbCapabilities();

  return useCallback((): AssistantTarget | null => {
    const { tabs, activeTabId } = useTabsStore.getState();
    const tab = activeTabId ? tabs.get(activeTabId) : undefined;
    const server = servers?.find(item => item.id === tab?.serverId);
    const supported = server
      ? capsByType[server.dbType]?.supportsAssistant === true
      : false;
    return tab && supported
      ? { serverId: tab.serverId, database: tab.database, schema: tab.schema }
      : null;
  }, [servers, capsByType]);
};

/**
 * Abre ou fecha o painel (botão da barra de abas e `Cmd/Ctrl+J`). Abrir leva o
 * banco da aba ativa como alvo.
 */
export const useToggleAssistant = () => {
  const activeTarget = useActiveTabTarget();
  const openPanel = useAssistantStore(state => state.openPanel);
  const closePanel = useAssistantStore(state => state.closePanel);

  return useCallback(() => {
    if (useAssistantStore.getState().panelOpen) {
      closePanel();
      return;
    }
    openPanel(activeTarget());
  }, [activeTarget, openPanel, closePanel]);
};

/**
 * Entra ou sai do modo agente (botão do painel e `Cmd/Ctrl+Shift+J`). Com o
 * painel fechado, entrar abre o painel como o `Cmd/Ctrl+J`.
 */
export const useToggleAgentMode = () => {
  const activeTarget = useActiveTabTarget();
  const setAgentMode = useAssistantStore(state => state.setAgentMode);

  return useCallback(() => {
    const { panelOpen, agentMode } = useAssistantStore.getState();
    if (panelOpen && agentMode) {
      setAgentMode(false);
      return;
    }
    setAgentMode(true, panelOpen ? undefined : activeTarget());
  }, [activeTarget, setAgentMode]);
};
