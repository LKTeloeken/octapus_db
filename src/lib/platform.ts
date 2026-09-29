/** macOS usa ⌘ nos atalhos; os demais sistemas, Ctrl. */
export const isMac =
  typeof navigator !== 'undefined' &&
  /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);

/** Rótulo da tecla modificadora para `Kbd` e tooltips (`⌘K` / `Ctrl K`). */
export const modKey = isMac ? '⌘' : 'Ctrl ';

/** Rótulo de um atalho com a modificadora: `shortcut('K')` → `⌘K`. */
export const shortcut = (key: string) => `${modKey}${key}`;
