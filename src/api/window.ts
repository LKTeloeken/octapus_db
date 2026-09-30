import type { UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';

/**
 * Ações da janela para a barra de título customizada. Não passam pelo
 * `call()`: são comandos do plugin de janela do Tauri, não do backend Rust.
 */

export const minimizeWindow = () => getCurrentWindow().minimize();

export const toggleMaximizeWindow = () => getCurrentWindow().toggleMaximize();

export const closeWindow = () => getCurrentWindow().close();

export const isWindowMaximized = () => getCurrentWindow().isMaximized();

/** O verde do semáforo no macOS: entra e sai da tela cheia nativa. */
export async function toggleWindowFullscreen(): Promise<void> {
  const window = getCurrentWindow();
  await window.setFullscreen(!(await window.isFullscreen()));
}

/** Dispara ao redimensionar — inclusive maximizar pelo WM ou por duplo clique. */
export const onWindowResized = (handler: () => void): Promise<UnlistenFn> =>
  getCurrentWindow().onResized(handler);
