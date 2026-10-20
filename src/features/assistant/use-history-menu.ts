import { useCallback, useState } from 'react';
import type { AssistantConversationSummary } from '@/api/types/assistant.types';
import { useAssistantConversations } from '@/queries/use-assistant-conversations';
import { useServers } from '@/queries/use-servers';
import {
  useActiveConversationTab,
  useAssistantStore,
} from '@/stores/assistant-store';

/**
 * Histórico do servidor (C-15). Reabrir mantém o alvo da aba ativa quando o
 * banco é do mesmo paradigma; senão a aba nasce sem alvo e a pessoa escolhe.
 */
export const useHistoryMenu = () => {
  const [open, setOpen] = useState(false);
  const conversations = useAssistantConversations(open);
  const { data: servers } = useServers();
  const activeTab = useActiveConversationTab();
  const openConversation = useAssistantStore(state => state.openConversation);

  const select = useCallback(
    (conversation: AssistantConversationSummary) => {
      const target = activeTab?.target ?? null;
      const server = servers?.find(item => item.id === target?.serverId);
      openConversation(
        conversation.id,
        server?.dbType === conversation.paradigm ? target : null,
      );
      setOpen(false);
    },
    [activeTab?.target, servers, openConversation],
  );

  return {
    open,
    setOpen,
    items: conversations.data?.items ?? [],
    isLoading: conversations.isPending && open,
    error: conversations.error,
    select,
  };
};
