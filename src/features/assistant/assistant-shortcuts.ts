/**
 * Atalhos do assistente (teclas no registro, `lib/keybindings/actions.ts`):
 *
 * - `Cmd/Ctrl+J` em qualquer lugar: abre/fecha o painel;
 * - `Cmd/Ctrl+Shift+J` em qualquer lugar: entra/sai do modo agente;
 * - com o foco **dentro** do painel, `Cmd/Ctrl+W` fecha a conversa ativa e
 *   `Cmd/Ctrl+T` abre uma nova — sem chegar aos atalhos das abas de query.
 *
 * O listener fica na fase de captura do `window` e chama `preventDefault`;
 * o `useShortcuts` das abas de query ignora evento já tratado.
 */

/** Marca o contêiner do painel: o foco dentro dele muda o `Cmd+W`/`Cmd+T` */
export const ASSISTANT_PANEL_ATTR = 'data-assistant-panel';

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
