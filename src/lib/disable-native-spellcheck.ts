const SPELLCHECK_OFF_ATTRS = {
  spellcheck: 'false',
  autocorrect: 'off',
  autocapitalize: 'off',
} as const;

function applySpellcheckOff(el: HTMLElement) {
  for (const [name, value] of Object.entries(SPELLCHECK_OFF_ATTRS)) {
    if (el.getAttribute(name) !== value) {
      el.setAttribute(name, value);
    }
  }
}

function isEditable(target: EventTarget | null): target is HTMLElement {
  return (
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    (target instanceof HTMLElement && target.isContentEditable)
  );
}

/**
 * Turns off macOS/WebKit spelling suggestions for every editable field.
 * `spellcheck` inherits from `<html>`; `autocorrect` / `autocapitalize` do not,
 * so they are applied when an input, textarea, or contenteditable receives focus.
 */
export function disableNativeSpellcheck() {
  document.documentElement.setAttribute('spellcheck', 'false');

  document.addEventListener('focusin', event => {
    if (isEditable(event.target)) {
      applySpellcheckOff(event.target);
    }
  });
}
