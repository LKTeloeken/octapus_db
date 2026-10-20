/**
 * Atalhos do assistente, decididos por uma função pura (testável sem DOM):
 *
 * - `Cmd/Ctrl+J` em qualquer lugar: abre/fecha o painel;
 * - com o foco **dentro** do painel, `Cmd/Ctrl+W` fecha a conversa ativa e
 *   `Cmd/Ctrl+T` abre uma nova — sem chegar aos atalhos das abas de query.
 *
 * O listener fica na fase de captura do `window` e chama `preventDefault`;
 * o `use-query-tabs` ignora evento já tratado (`defaultPrevented`).
 */

export type AssistantShortcut = 'toggle-panel' | 'close-tab' | 'new-tab';

/** Marca o contêiner do painel: o foco dentro dele muda o `Cmd+W`/`Cmd+T` */
export const ASSISTANT_PANEL_ATTR = 'data-assistant-panel';

export interface ShortcutKey {
  key: string;
  metaKey: boolean;
  ctrlKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
}

export function assistantShortcut(
  event: ShortcutKey,
  focusInPanel: boolean,
): AssistantShortcut | null {
  if (!(event.metaKey || event.ctrlKey) || event.shiftKey || event.altKey) {
    return null;
  }

  const key = event.key.toLowerCase();
  if (key === 'j') return 'toggle-panel';
  if (!focusInPanel) return null;
  if (key === 'w') return 'close-tab';
  if (key === 't') return 'new-tab';
  return null;
}

/** O alvo do evento (ou o foco) está dentro do painel do assistente */
export function isInAssistantPanel(target: EventTarget | null): boolean {
  const element =
    target instanceof Element
      ? target
      : typeof document !== 'undefined'
        ? document.activeElement
        : null;
  return !!element?.closest(`[${ASSISTANT_PANEL_ATTR}]`);
}
