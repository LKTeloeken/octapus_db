import { useCallback, useState } from 'react';
import type { AssistantConversationSummary } from '@/api/types/assistant.types';
import { useAssistantConversations } from '@/queries/use-assistant-conversations';
import { useServers } from '@/queries/use-servers';
import {
  useActiveConversationTab,
  useAssistantStore,
} from '@/stores/assistant-store';

/**
 * Histórico do servidor (C-15), pedido só com `enabled`. Reabrir mantém o
 * alvo da aba ativa quando o banco é do mesmo paradigma; senão a aba nasce
 * sem alvo e a pessoa escolhe.
 */
export const useConversationHistory = (enabled: boolean) => {
  const conversations = useAssistantConversations(enabled);
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
    },
    [activeTab?.target, servers, openConversation],
  );

  return {
    items: conversations.data?.items ?? [],
    isLoading: conversations.isPending && enabled,
    error: conversations.error,
    select,
  };
};

/** O histórico no menu do cabeçalho: pede a lista ao abrir */
export const useHistoryMenu = () => {
  const [open, setOpen] = useState(false);
  const history = useConversationHistory(open);

  const select = useCallback(
    (conversation: AssistantConversationSummary) => {
      history.select(conversation);
      setOpen(false);
    },
    [history],
  );

  return { open, setOpen, ...history, select };
};
