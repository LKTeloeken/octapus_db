import { useCallback, useEffect, useRef } from 'react';
import { useShortcuts } from '@/lib/keybindings/use-shortcuts';
import { isCompactWindow } from '@/lib/window-size';
import { useCancelAssistantTurn } from '@/queries/use-assistant-conversations';
import { useAssistantStatus } from '@/queries/use-assistant-status';
import { useAssistantStore } from '@/stores/assistant-store';
import { useTabsStore } from '@/stores/tabs-store';
import { isInAssistantPanel } from './assistant-shortcuts';
import { useToggleAgentMode, useToggleAssistant } from './use-toggle-assistant';

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
  const toggleAgentMode = useToggleAgentMode();
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
    window.addEventListener('focusin', track, { capture: true });
    window.addEventListener('pointerdown', track, { capture: true });
    return () => {
      window.removeEventListener('focusin', track, { capture: true });
      window.removeEventListener('pointerdown', track, { capture: true });
    };
  }, [configured]);

  /** O evento é do painel aberto (ou, com o foco solto, foi a última zona) */
  const inPanel = (event: { target: EventTarget | null }) => {
    const { target } = event;
    const loose =
      target === document.body || target === document.documentElement;
    return (
      useAssistantStore.getState().panelOpen &&
      (loose ? lastZoneInPanel.current : isInAssistantPanel(target))
    );
  };

  // Captura + preventDefault: as abas de query ignoram evento tratado
  useShortcuts(
    {
      'assistant.togglePanel': togglePanel,
      'assistant.toggleAgentMode': toggleAgentMode,
      'assistant.newConversation': event => {
        if (!inPanel(event)) return false;
        newTab();
      },
      'assistant.closeConversation': event => {
        if (!inPanel(event)) return false;
        const { activeTabId } = useAssistantStore.getState();
        if (activeTabId) closeConversation(activeTabId);
      },
    },
    { capture: true, enabled: configured },
  );

  // Uma aba de query aberta ou ativada (paleta, "Abrir no editor", "Ver
  // definição") só se vê fora do modo agente: ele sai sozinho. Na janela
  // compacta não há modo sem ser o agente: o painel fecha (as conversas ficam)
  useEffect(
    () =>
      useTabsStore.subscribe((state, previous) => {
        if (state.activeTabId === previous.activeTabId) return;
        const { panelOpen, agentMode, setAgentMode, closePanel } =
          useAssistantStore.getState();
        if (panelOpen && isCompactWindow()) closePanel();
        else if (agentMode) setAgentMode(false);
      }),
    [],
  );
};
