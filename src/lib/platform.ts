/** macOS usa ⌘ nos atalhos; os demais sistemas, Ctrl. */
export const isMac =
  typeof navigator !== 'undefined' &&
  /Mac|iPhone|iPad/.test(navigator.platform || navigator.userAgent);

export const isLinux =
  !isMac &&
  typeof navigator !== 'undefined' &&
  /Linux/.test(navigator.platform || navigator.userAgent) &&
  !/Android/.test(navigator.userAgent);

/**
 * Qual barra de título o front desenha. macOS: semáforo à esquerda, com a
 * barra nativa em Overlay (`tauri.macos.conf.json`). Linux: controles à
 * direita da linha de abas, janela sem decoração (`tauri.linux.conf.json`).
 * Windows segue com a barra do sistema.
 */
export const customTitlebar: 'mac' | 'linux' | null = isMac
  ? 'mac'
  : isLinux
    ? 'linux'
    : null;
