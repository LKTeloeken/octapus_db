import { create } from 'zustand';

/**
 * O que está rodando agora, por aba — o sinal de vida do Abissal: o ponto
 * biolume na aba, o chip de conexão pulsando e o polvo trabalhando.
 */
interface ActivityState {
  runningTabs: ReadonlySet<string>;
  setRunning: (tabId: string, running: boolean) => void;
}

export const useActivityStore = create<ActivityState>(set => ({
  runningTabs: new Set(),
  setRunning: (tabId, running) =>
    set(state => {
      if (state.runningTabs.has(tabId) === running) return state;
      const next = new Set(state.runningTabs);
      if (running) next.add(tabId);
      else next.delete(tabId);
      return { runningTabs: next };
    }),
}));

export const useIsTabRunning = (tabId: string) =>
  useActivityStore(state => state.runningTabs.has(tabId));

export const useAnyRunning = () =>
  useActivityStore(state => state.runningTabs.size > 0);
