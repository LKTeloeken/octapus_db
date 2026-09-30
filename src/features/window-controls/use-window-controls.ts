import { useCallback, useEffect, useState } from 'react';
import {
  closeWindow,
  isWindowMaximized,
  minimizeWindow,
  onWindowResized,
  toggleMaximizeWindow,
  toggleWindowFullscreen,
} from '@/api/window';

// Falhas aqui não têm o que mostrar ao usuário (a janela simplesmente não
// muda); só registram para não virar rejeição solta.
const report = (error: unknown) => console.error('[window-controls]', error);

/**
 * Controles do Linux. Acompanha o estado maximizado para trocar o ícone —
 * inclusive quando quem maximiza é o WM ou o duplo clique na região de arrasto.
 */
export const useWindowControls = () => {
  const [isMaximized, setIsMaximized] = useState(false);

  const syncMaximized = useCallback(() => {
    isWindowMaximized().then(setIsMaximized).catch(report);
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let cancelled = false;

    syncMaximized();
    onWindowResized(syncMaximized)
      .then(fn => {
        if (cancelled) fn();
        else unlisten = fn;
      })
      .catch(report);

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [syncMaximized]);

  const minimize = useCallback(() => {
    minimizeWindow().catch(report);
  }, []);

  const toggleMaximize = useCallback(() => {
    toggleMaximizeWindow().then(syncMaximized).catch(report);
  }, [syncMaximized]);

  const close = useCallback(() => {
    closeWindow().catch(report);
  }, []);

  return { isMaximized, minimize, toggleMaximize, close };
};

/** Semáforo do macOS: o verde alterna a tela cheia nativa, como no sistema. */
export const useTrafficLights = () => {
  const minimize = useCallback(() => {
    minimizeWindow().catch(report);
  }, []);

  const toggleFullscreen = useCallback(() => {
    toggleWindowFullscreen().catch(report);
  }, []);

  const close = useCallback(() => {
    closeWindow().catch(report);
  }, []);

  return { minimize, toggleFullscreen, close };
};
