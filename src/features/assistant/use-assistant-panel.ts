import { useCallback, useMemo } from 'react';
import { useAssistantConversations } from '@/queries/use-assistant-conversations';
import { useAssistantMe } from '@/queries/use-assistant-me';
import { useAssistantStatus } from '@/queries/use-assistant-status';
import { useCompactWindow } from '@/lib/window-size';
import { useAssistantStore } from '@/stores/assistant-store';
import { assistantGate, canCallApi } from './assistant-gate';
import { useCloseConversation } from './use-assistant-shortcuts';
import { useToggleAgentMode } from './use-toggle-assistant';

/** Rótulo de uma aba de conversa ainda sem título no servidor */
const NEW_CONVERSATION_LABEL = 'Nova conversa';

export const useAssistantPanel = () => {
  const status = useAssistantStatus();
  const me = useAssistantMe(canCallApi(status.data));
  const gate = assistantGate({
    status: status.data,
    me: me.data,
    meError: me.error,
  });
  const ready = gate.kind === 'ready';

  const tabs = useAssistantStore(state => state.tabs);
  const activeTabId = useAssistantStore(state => state.activeTabId);
  const live = useAssistantStore(state => state.live);
  const setActiveTab = useAssistantStore(state => state.setActiveTab);
  const closeTab = useCloseConversation();
  const newTab = useAssistantStore(state => state.newTab);
  const closePanel = useAssistantStore(state => state.closePanel);
  // Janela compacta: modo agente sempre, sem como sair dele (só fechar)
  const compact = useCompactWindow();
  const agentMode = useAssistantStore(state => state.agentMode) || compact;
  const toggleAgentMode = useToggleAgentMode();

  // Os títulos vêm da lista do servidor (a API dá o título na 1ª pergunta)
  const conversations = useAssistantConversations(ready);
  const titles = useMemo(
    () =>
      new Map(
        (conversations.data?.items ?? []).map(item => [item.id, item.title]),
      ),
    [conversations.data],
  );

  const tabItems = useMemo(
    () =>
      tabs.map(tab => {
        const title =
          (tab.conversationId ? titles.get(tab.conversationId) : null) ??
          live[tab.id]?.question ??
          NEW_CONVERSATION_LABEL;
        return {
          id: tab.id,
          conversationId: tab.conversationId,
          title,
          running: !!live[tab.id] && live[tab.id].phase !== 'finished',
        };
      }),
    [tabs, titles, live],
  );

  const activeTab = tabs.find(tab => tab.id === activeTabId) ?? null;

  const retry = useCallback(() => {
    void status.refetch();
    if (canCallApi(status.data)) void me.refetch();
  }, [status, me]);

  return {
    gate,
    tabs: tabItems,
    activeTab,
    activeTabId,
    setActiveTab,
    closeTab,
    newTab: useCallback(() => void newTab(), [newTab]),
    closePanel,
    agentMode,
    canToggleAgentMode: !compact,
    toggleAgentMode,
    retry,
    isRetrying: status.isFetching || me.isFetching,
  };
};
