import { useSyncExternalStore } from 'react';

/**
 * Abaixo desta largura a janela é "compacta": não cabem barra lateral, abas e
 * assistente lado a lado, e o assistente abre direto no modo agente.
 */
export const COMPACT_WINDOW_PX = 1040;

export const isCompactWindow = (width = window.innerWidth): boolean =>
  width < COMPACT_WINDOW_PX;

const subscribe = (onChange: () => void) => {
  window.addEventListener('resize', onChange);
  return () => window.removeEventListener('resize', onChange);
};

/** Largura da janela, acompanhando o redimensionamento */
export const useWindowWidth = (): number =>
  useSyncExternalStore(subscribe, () => window.innerWidth);

export const useCompactWindow = (): boolean =>
  isCompactWindow(useWindowWidth());
