import { useCallback, useEffect, useRef } from 'react';
import { useCancelAssistantTurn } from '@/queries/use-assistant-conversations';
import { useAssistantStatus } from '@/queries/use-assistant-status';
import { useAssistantStore } from '@/stores/assistant-store';
import { assistantShortcut, isInAssistantPanel } from './assistant-shortcuts';
import { useToggleAssistant } from './use-toggle-assistant';

/** Fecha uma aba de conversa, cancelando o turno dela se ainda roda (C-18) */
export const useCloseConversation = () => {
  const cancel = useCancelAssistantTurn();
  const closeTab = useAssistantStore(state => state.closeTab);

  return useCallback(
    (tabId: string) => {
      const turn = useAssistantStore.getState().live[tabId];
      if (turn && turn.phase !== 'finished') cancel.mutate(turn.clientTurnId);
      closeTab(tabId);
    },
    [cancel, closeTab],
  );
};

/**
 * Atalhos do assistente (ver `assistant-shortcuts.ts`). Montado sempre, com o
 * painel aberto ou não: o `Cmd/Ctrl+J` precisa funcionar para abri-lo. Sem o
 * assistente no build, nem o `J` é tomado.
 */
export const useAssistantShortcuts = () => {
  const { data: status } = useAssistantStatus();
  const configured = status?.configured === true;
  const togglePanel = useToggleAssistant();
  const newTab = useAssistantStore(state => state.newTab);
  const closeConversation = useCloseConversation();

  // Onde a pessoa clicou ou focou por último. Fechar uma conversa remonta o
  // campo e o foco cai no `body`: aí vale a última zona, senão o próximo
  // Cmd/Ctrl+W fecharia uma aba de query
  const lastZoneInPanel = useRef(false);

  useEffect(() => {
    if (!configured) return;

    const track = (event: Event) => {
      if (event.target === document.body) return;
      lastZoneInPanel.current = isInAssistantPanel(event.target);
    };
    const handler = (event: KeyboardEvent) => {
      const target = event.target;
      const loose =
        target === document.body || target === document.documentElement;
      const inPanel =
        useAssistantStore.getState().panelOpen &&
        (loose ? lastZoneInPanel.current : isInAssistantPanel(target));
      const action = assistantShortcut(event, inPanel);
      if (!action) return;
      // Captura + preventDefault: as abas de query ignoram evento tratado
      event.preventDefault();

      if (action === 'toggle-panel') {
        togglePanel();
        return;
      }
      if (action === 'new-tab') {
        newTab();
        return;
      }
      const { activeTabId } = useAssistantStore.getState();
      if (activeTabId) closeConversation(activeTabId);
    };

    window.addEventListener('keydown', handler, { capture: true });
    window.addEventListener('focusin', track, { capture: true });
    window.addEventListener('pointerdown', track, { capture: true });
    return () => {
      window.removeEventListener('keydown', handler, { capture: true });
      window.removeEventListener('focusin', track, { capture: true });
      window.removeEventListener('pointerdown', track, { capture: true });
    };
  }, [configured, togglePanel, newTab, closeConversation]);
};
