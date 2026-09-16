import { create } from 'zustand';
import { persist } from 'zustand/middleware';

export type FailMode = 'off' | 'next' | 'always';

export const DEFAULT_ERROR_MESSAGE =
  'Connection error: connection refused (mock)';

interface MockState {
  /** Latência artificial de cada comando, em ms */
  latencyMs: number;
  failMode: FailMode;
  errorMessage: string;
  /** Listagens devolvem vazio — para desenhar empty states */
  emptyMode: boolean;
  /** `plugin:updater|check` devolve um update fake */
  updateAvailable: boolean;
  panelOpen: boolean;

  setLatency: (ms: number) => void;
  setFailMode: (mode: FailMode) => void;
  setErrorMessage: (message: string) => void;
  setEmptyMode: (enabled: boolean) => void;
  setUpdateAvailable: (enabled: boolean) => void;
  setPanelOpen: (open: boolean) => void;

  /**
   * Devolve a mensagem de erro quando o próximo comando deve falhar — e, no
   * modo 'next', já se desarma. Chamado pelos handlers, fora do React.
   */
  consumeFailure: () => string | null;
}

export const useMockStore = create<MockState>()(
  persist(
    (set, get) => ({
      latencyMs: 250,
      failMode: 'off',
      errorMessage: DEFAULT_ERROR_MESSAGE,
      emptyMode: false,
      updateAvailable: false,
      panelOpen: false,

      setLatency: latencyMs => set({ latencyMs }),
      setFailMode: failMode => set({ failMode }),
      setErrorMessage: errorMessage => set({ errorMessage }),
      setEmptyMode: emptyMode => set({ emptyMode }),
      setUpdateAvailable: updateAvailable => set({ updateAvailable }),
      setPanelOpen: panelOpen => set({ panelOpen }),

      consumeFailure: () => {
        const { failMode, errorMessage } = get();
        if (failMode === 'off') return null;
        if (failMode === 'next') set({ failMode: 'off' });
        return errorMessage;
      },
    }),
    {
      name: 'octapus-mock',
      // `failMode: 'next'` não sobrevive ao reload de propósito: seria uma
      // armadilha reabrir o app já quebrado.
      partialize: state => ({
        latencyMs: state.latencyMs,
        errorMessage: state.errorMessage,
        emptyMode: state.emptyMode,
        updateAvailable: state.updateAvailable,
        panelOpen: state.panelOpen,
      }),
    },
  ),
);
