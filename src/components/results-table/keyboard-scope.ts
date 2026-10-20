/**
 * Regiões de teclado. Os atalhos da grade (Cmd/Ctrl+S, A, Z, F, Backspace, Esc)
 * escutam o `window`, o que só funciona com uma grade na tela. O assistente
 * mostra várias (uma por resposta) ao lado da aba ativa, então:
 *
 * - um contêiner com `data-keyboard-scope="<id>"` é uma região;
 * - a grade da aba (sem região própria) atende só eventos de fora de qualquer
 *   região — o `Cmd+S` digitado no assistente não salva a aba;
 * - a grade com `keyboardScope` vira uma região e atende só o que nasce nela.
 */
export const KEYBOARD_SCOPE_ATTR = 'data-keyboard-scope';

/** A região mais interna que contém o alvo do evento, ou `null` */
export function keyboardScopeOf(target: EventTarget | null): string | null {
  if (typeof Element === 'undefined' || !(target instanceof Element)) {
    return null;
  }
  return (
    target
      .closest(`[${KEYBOARD_SCOPE_ATTR}]`)
      ?.getAttribute(KEYBOARD_SCOPE_ATTR) ?? null
  );
}

/** A grade da região `ownScope` (`null` = sem região) atende este evento */
export const ownsKeyboardEvent = (
  target: EventTarget | null,
  ownScope: string | null,
) => keyboardScopeOf(target) === ownScope;
